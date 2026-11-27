use crate::{clipboard::wide, images, settings::Settings, store::write_file};
use anyhow::{Result, ensure};
use std::{
    env, fs, io,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    ptr::{null, null_mut},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
    },
    System::{
        Registry::*,
        Threading::{
            CREATE_NO_WINDOW, CreateEventW, CreateMutexW, ReleaseMutex, SetEvent,
            WaitForSingleObject,
        },
    },
};

const WORKER: &str = "ThodClip.Worker.exe";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[derive(Clone)]
pub struct Integration {
    pub root: PathBuf,
    pub home: PathBuf,
}

struct Handle(HANDLE);

impl Handle {
    fn new(raw: HANDLE) -> Result<Self> {
        ensure!(!raw.is_null(), io::Error::last_os_error());
        Ok(Self(raw))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub struct Lease {
    handle: Handle,
    acquired: bool,
}

impl Lease {
    pub fn acquire(&mut self) -> bool {
        if !self.acquired {
            self.acquired = matches!(
                unsafe { WaitForSingleObject(self.handle.0, 0) },
                WAIT_OBJECT_0 | WAIT_ABANDONED
            );
        }
        self.acquired
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.acquired {
            unsafe {
                ReleaseMutex(self.handle.0);
            }
        }
    }
}

pub struct StopEvent(Handle);

impl StopEvent {
    pub fn signaled(&self) -> bool {
        unsafe { WaitForSingleObject(self.0.0, 0) == WAIT_OBJECT_0 }
    }
}

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn registry_result(code: u32) -> Result<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code as i32).into())
    }
}

impl Integration {
    fn instance_name(&self, role: &str) -> Vec<u16> {
        let root = self.root.to_string_lossy().replace('/', "\\");
        let hash = images::hash(root.trim_end_matches('\\').to_uppercase());
        wide(format!(
            r"Local\ThodClip.{role}.{}",
            hash[..16].to_uppercase()
        ))
    }

    pub fn lease(&self, role: &str) -> Result<Lease> {
        let handle =
            Handle::new(unsafe { CreateMutexW(null(), 0, self.instance_name(role).as_ptr()) })?;
        Ok(Lease {
            handle,
            acquired: false,
        })
    }

    pub fn stop_event(&self) -> Result<StopEvent> {
        Ok(StopEvent(Handle::new(unsafe {
            CreateEventW(null(), 1, 0, self.instance_name("Background.Stop").as_ptr())
        })?))
    }

    pub fn background_running(&self) -> bool {
        let Ok(mut lease) = self.lease("Background") else {
            return false;
        };
        !lease.acquire()
    }

    pub fn stop(&self) -> Result<()> {
        let event = self.stop_event()?;
        ensure!(
            unsafe { SetEvent(event.0.0) } != 0,
            io::Error::last_os_error()
        );
        let deadline = Instant::now() + Duration::from_secs(8);
        while self.background_running() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
        ensure!(
            !self.background_running(),
            "Фоновая запись не остановилась. Перезапустите компьютер и повторите."
        );
        Ok(())
    }

    fn cache(&self) -> PathBuf {
        self.home.join("Cache").join("thodclip")
    }
    fn stable(&self) -> PathBuf {
        self.cache().join("worker").join(WORKER)
    }
    pub fn log_path(&self) -> PathBuf {
        self.cache().join("background.log")
    }

    fn register(&self) -> Result<()> {
        let mut key = null_mut();
        registry_result(unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                null(),
                &mut key,
                null_mut(),
            )
        })?;
        let key = RegistryKey(key);
        let value = wide(format!(
            "\"{}\" --background --home \"{}\" --data \"{}\"",
            self.stable().display(),
            self.home.display(),
            self.root.display()
        ));
        registry_result(unsafe {
            RegSetValueExW(
                key.0,
                wide("ThodClip").as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        })
    }

    fn unregister(&self) -> Result<()> {
        let mut key = null_mut();
        let code = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                &mut key,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        registry_result(code)?;
        let key = RegistryKey(key);
        let name = wide("ThodClip");
        let mut value = vec![0u16; 32768];
        let mut bytes = (value.len() * 2) as u32;
        let mut kind = 0;
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                null(),
                &mut kind,
                value.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        registry_result(code)?;
        let value = String::from_utf16_lossy(&value[..(bytes as usize / 2).min(value.len())])
            .to_lowercase();
        let expected = format!("\"{}\" --background ", self.stable().display()).to_lowercase();
        if kind == REG_SZ && value.starts_with(&expected) {
            registry_result(unsafe { RegDeleteValueW(key.0, name.as_ptr()) })?;
        }
        Ok(())
    }

    fn ensure_stable(&self) -> Result<()> {
        let source = env::current_exe()?;
        let stable = self.stable();
        if source == stable {
            return Ok(());
        }
        let exe = fs::read(&source)?;
        if fs::read(&stable).is_ok_and(|cached| cached == exe) {
            return Ok(());
        }
        self.stop()?;
        write_file(&stable, &exe)
    }

    fn start(&self) -> Result<()> {
        if self.background_running() {
            return Ok(());
        }
        let path = self.stable();
        let parent = path.parent().expect("worker directory");
        fs::create_dir_all(self.cache())?;
        let log_path = self.log_path();
        if fs::metadata(&log_path).is_ok_and(|info| info.len() > 1_000_000) {
            let _ = fs::rename(&log_path, log_path.with_extension("old.log"));
        }
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;
        Command::new(&path)
            .creation_flags(CREATE_NO_WINDOW)
            .current_dir(parent)
            .arg("--background")
            .arg("--home")
            .arg(&self.home)
            .arg("--data")
            .arg(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            if self.background_running() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        anyhow::bail!(
            "Фоновый процесс не запустился. Журнал: {}",
            self.log_path().display()
        )
    }

    pub fn apply(&self, enabled: bool, start: bool) -> Result<()> {
        if enabled {
            self.ensure_stable()?;
            self.register()?;
            if start {
                self.start()?;
            }
        } else {
            self.unregister()?;
            self.stop()?;
        }
        Ok(())
    }

    pub fn synchronize(&self) -> Result<()> {
        self.apply(Settings::load(&self.root).run_in_background, false)
    }

    pub fn remove(&self) -> Result<()> {
        self.apply(false, false)?;
        let cache = self.cache();
        if cache.is_dir() {
            fs::remove_dir_all(cache)?;
        }
        Ok(())
    }
}

pub fn reveal(path: &Path, select: bool) -> Result<()> {
    let mut explorer = Command::new("explorer.exe");
    if select {
        explorer.raw_arg(format!("/select,\"{}\"", path.display()));
    } else {
        explorer.arg(path);
    }
    explorer.spawn()?;
    Ok(())
}
