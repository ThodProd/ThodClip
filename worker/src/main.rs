mod clipboard;
mod engine;
mod gdiplus;
mod images;
mod integration;
mod screenshot_watch;
mod screenshots;
mod settings;
mod store;

use anyhow::{Result, ensure};
use engine::Engine;
use integration::Integration;
use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, BufRead, Write},
    path::PathBuf,
    sync::mpsc,
    thread,
    time::Duration,
};

fn argument(args: &[OsString], options: &[&str]) -> Result<Option<PathBuf>> {
    for (index, arg) in args.iter().enumerate() {
        let arg = arg.to_string_lossy();
        for option in options {
            if arg.eq_ignore_ascii_case(option) {
                let path = args
                    .get(index + 1)
                    .filter(|value| !value.to_string_lossy().trim().is_empty());
                return Ok(Some(PathBuf::from(path.ok_or_else(|| {
                    anyhow::anyhow!("Не указан каталог после {option}.")
                })?)));
            }
            if let Some((name, path)) = arg.split_once('=')
                && name.eq_ignore_ascii_case(option)
            {
                ensure!(!path.trim().is_empty(), "Не указан каталог после {option}.");
                return Ok(Some(PathBuf::from(path)));
            }
        }
    }
    Ok(None)
}

fn run() -> Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let home = argument(&args, &["--home", "--thodhub-home"])?
        .or_else(|| {
            env::var_os("THODHUB_HOME")
                .filter(|arg| !arg.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| screenshots::local_app_data().join("ThodHUB"));
    let root = argument(&args, &["--data", "--thodhub-data"])?
        .or_else(|| {
            env::var_os("THODHUB_DATA_DIR")
                .filter(|arg| !arg.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| screenshots::local_app_data().join("ThodClip"));
    let integration = Integration {
        home: std::path::absolute(home)?,
        root: std::path::absolute(root)?,
    };
    let has = |flag: &str| args.iter().any(|arg| arg.eq_ignore_ascii_case(flag));
    if has("--register-integrations") {
        return integration.synchronize();
    }
    if has("--uninstall-integrations") {
        return integration.remove();
    }
    if has("--stop-background") {
        return integration.stop();
    }
    fs::create_dir_all(&integration.root)?;
    if has("--background") {
        let event = integration.stop_event()?;
        let mut lease = integration.lease("Background")?;
        if !lease.acquire()
            || event.signaled()
            || !settings::Settings::load(&integration.root).run_in_background
        {
            return Ok(());
        }
        let mut engine = Engine::new(integration, true)?;
        engine.maintenance();
        while !event.signaled() {
            match engine.tick() {
                Ok(false) => break,
                Ok(true) => (),
                Err(error) => eprintln!("capture: {error:#}"),
            }
            thread::sleep(Duration::from_millis(35));
        }
        return Ok(());
    }

    let mut engine = Engine::new(integration, false)?;
    engine.maintenance();
    engine.verify_background();
    let (sender, receiver) = mpsc::sync_channel(64);
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let stdout = io::stdout();
    let mut output = stdout.lock();
    loop {
        if let Err(error) = engine.tick() {
            eprintln!("capture: {error:#}");
        }
        match receiver.recv_timeout(Duration::from_millis(35)) {
            Ok(Ok(line)) => {
                if line.trim().is_empty() {
                    continue;
                }
                if let Some(response) = engine.response(line.trim_start_matches('\u{feff}')) {
                    serde_json::to_writer(&mut output, &response)?;
                    output.write_all(b"\n")?;
                    output.flush()?;
                }
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ThodClip: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
