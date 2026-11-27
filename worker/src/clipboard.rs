use crate::{images, screenshots};
use anyhow::{Result, ensure};
use std::{
    ffi::OsStr,
    io,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HWND},
    Graphics::Gdi::{BI_RGB, BITMAP, BITMAPINFO, DIB_RGB_COLORS, GetDIBits, GetObjectW},
    System::{
        DataExchange::*,
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock},
        Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
    },
    UI::{
        Shell::{DROPFILES, DragQueryFileW, HDROP},
        WindowsAndMessaging::*,
    },
};

const TEXT: u32 = 13;
const BITMAP_FORMAT: u32 = 2;
const FILES: u32 = 15;
const DIB: u32 = 8;
const DIBV5: u32 = 17;
const MAX_CLIPBOARD_BYTES: usize = 512 * 1024 * 1024;

pub enum Kind {
    Text(String),
    Files(Vec<String>),
    Image {
        png: Vec<u8>,
        width: u32,
        height: u32,
        files: Vec<String>,
    },
}

pub struct Clipboard {
    window: HWND,
    suppress_until: Instant,
}

pub fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain([0]).collect()
}

fn register(format: &str) -> u32 {
    unsafe { RegisterClipboardFormatW(wide(format).as_ptr()) }
}

fn error() -> anyhow::Error {
    io::Error::last_os_error().into()
}

struct Open;

impl Open {
    fn acquire(window: HWND) -> Result<Self> {
        for _ in 0..6 {
            if unsafe { OpenClipboard(window) } != 0 {
                return Ok(Self);
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(error())
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

impl Clipboard {
    pub fn new() -> Result<Self> {
        // Use a message-only window so it never appears on the desktop or taskbar.
        let window = unsafe {
            CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                null_mut(),
                null(),
            )
        };
        ensure!(!window.is_null(), error());
        if unsafe { AddClipboardFormatListener(window) } == 0 {
            let err = error();
            unsafe {
                DestroyWindow(window);
            }
            return Err(err);
        }
        Ok(Self {
            window,
            suppress_until: Instant::now(),
        })
    }

    pub fn poll(&self, capture: bool, honor_exclusions: bool) -> Option<(Kind, Option<String>)> {
        let mut changed = false;
        unsafe {
            let mut message = zeroed();
            while PeekMessageW(&mut message, self.window, 0, 0, PM_REMOVE) != 0 {
                if message.message == WM_CLIPBOARDUPDATE {
                    changed = true;
                } else {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        if !changed || !capture || Instant::now() < self.suppress_until {
            return None;
        }
        let (snapshot, source) = {
            let _open = Open::acquire(self.window).ok()?;
            if honor_exclusions && excluded() {
                return None;
            }
            let source = source();
            let files = read_files();
            let snapshot = if !files.is_empty() {
                Snapshot::Files(files)
            } else {
                let png = bytes(register("PNG"));
                let dib = if png
                    .as_deref()
                    .is_some_and(|bytes| bytes.starts_with(b"\x89PNG\r\n\x1a\n"))
                {
                    None
                } else {
                    bytes(DIBV5)
                        .or_else(|| bytes(DIB))
                        .and_then(dib_to_bmp)
                        .or_else(read_bitmap)
                };
                Snapshot::Data {
                    png,
                    dib,
                    html: bytes(register("HTML Format")),
                    text: unicode(),
                }
            };
            (snapshot, source)
        };
        let kind = match snapshot {
            Snapshot::Files(mut files) => {
                if files.len() == 1
                    && images::is_image(Path::new(&files[0]))
                    && let Ok((png, width, height)) = images::file_png(Path::new(&files[0]))
                {
                    Kind::Image {
                        png,
                        width,
                        height,
                        files: std::mem::take(&mut files),
                    }
                } else {
                    Kind::Files(files)
                }
            }
            Snapshot::Data {
                png,
                dib,
                html,
                text,
            } => {
                let image = png
                    .filter(|bytes| bytes.starts_with(b"\x89PNG\r\n\x1a\n"))
                    .and_then(|png| {
                        let (width, height) =
                            images::Picture::open(&png).ok()?.dimensions().ok()?;
                        Some((png, width, height, vec![]))
                    })
                    .or_else(|| {
                        let image = images::Picture::open(&dib?).ok()?;
                        let (width, height) = image.dimensions().ok()?;
                        Some((image.png().ok()?, width, height, vec![]))
                    })
                    .or_else(|| screenshots::image_from_html(&String::from_utf8_lossy(&html?)))
                    .or_else(|| {
                        let path = screenshots::image_reference(text.as_deref()?)?;
                        let (png, width, height) = images::file_png(&path).ok()?;
                        Some((
                            png,
                            width,
                            height,
                            vec![path.to_string_lossy().into_owned()],
                        ))
                    });
                if let Some((png, width, height, files)) = image {
                    Kind::Image {
                        png,
                        width,
                        height,
                        files,
                    }
                } else {
                    Kind::Text(text.filter(|text| !text.trim().is_empty())?)
                }
            }
        };
        Some((kind, source))
    }

    fn set(&mut self, formats: &[(u32, &[u8])]) -> Result<()> {
        let _open = Open::acquire(self.window)?;
        self.suppress_until = Instant::now() + Duration::from_millis(1600);
        ensure!(unsafe { EmptyClipboard() } != 0, error());
        for &(format, bytes) in formats {
            unsafe {
                let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
                ensure!(!handle.is_null(), error());
                let data = GlobalLock(handle);
                if data.is_null() {
                    let err = error();
                    GlobalFree(handle);
                    return Err(err);
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast(), bytes.len());
                GlobalUnlock(handle);
                if SetClipboardData(format, handle).is_null() {
                    let err = error();
                    GlobalFree(handle);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub fn text(&mut self, text: &str) -> Result<()> {
        let bytes = wide(text)
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        self.set(&[(TEXT, &bytes)])
    }

    pub fn files(&mut self, paths: &[String]) -> Result<()> {
        let mut bytes = Vec::new();
        let header: DROPFILES = DROPFILES {
            pFiles: size_of::<DROPFILES>() as u32,
            pt: Default::default(),
            fNC: 0,
            fWide: 1,
        };
        bytes.extend(header.pFiles.to_le_bytes());
        bytes.extend([0; 12]);
        bytes.extend(header.fWide.to_le_bytes());
        for path in paths {
            bytes.extend(wide(path).into_iter().flat_map(u16::to_le_bytes));
        }
        bytes.extend([0; 2]);
        self.set(&[(FILES, &bytes)])
    }

    pub fn image(&mut self, path: &Path) -> Result<()> {
        let input = std::fs::read(path)?;
        let image = images::decode_raster(&input)?;
        let png = images::Picture::open(&input)?.png()?;
        let rgba = image.to_rgba8();
        let mut dib = bitmap_header(rgba.width(), rgba.height(), 32);
        for y in (0..rgba.height()).rev() {
            for x in 0..rgba.width() {
                let [r, g, b, a] = rgba.get_pixel(x, y).0;
                dib.extend([b, g, r, a]);
            }
        }
        self.set(&[(register("PNG"), &png), (DIB, &dib)])
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        unsafe {
            RemoveClipboardFormatListener(self.window);
            DestroyWindow(self.window);
        }
    }
}

enum Snapshot {
    Files(Vec<String>),
    Data {
        png: Option<Vec<u8>>,
        dib: Option<Vec<u8>>,
        html: Option<Vec<u8>>,
        text: Option<String>,
    },
}

fn bytes(format: u32) -> Option<Vec<u8>> {
    unsafe {
        let handle = GetClipboardData(format);
        if handle.is_null() {
            return None;
        }
        let size = GlobalSize(handle);
        if size == 0 || size > MAX_CLIPBOARD_BYTES {
            return None;
        }
        let data = GlobalLock(handle);
        if data.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec();
        GlobalUnlock(handle);
        Some(bytes)
    }
}

fn unicode() -> Option<String> {
    let bytes = bytes(TEXT)?;
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_le_bytes(pair))
        .take_while(|&unit| unit != 0)
        .collect::<Vec<_>>();
    Some(String::from_utf16_lossy(&units))
}

fn excluded() -> bool {
    let format = register("ExcludeClipboardContentFromMonitorProcessing");
    (unsafe { IsClipboardFormatAvailable(format) != 0 })
        || ["CanIncludeInClipboardHistory", "CanUploadToCloudClipboard"]
            .iter()
            .any(|format| bytes(register(format)).is_some_and(|data| data.starts_with(&[0; 4])))
}

fn read_files() -> Vec<String> {
    unsafe {
        let handle = GetClipboardData(FILES) as HDROP;
        if handle.is_null() {
            return vec![];
        }
        let count = DragQueryFileW(handle, u32::MAX, null_mut(), 0).min(500);
        (0..count)
            .filter_map(|index| {
                let length = DragQueryFileW(handle, index, null_mut(), 0);
                let mut buffer = vec![0; length as usize + 1];
                let read = DragQueryFileW(handle, index, buffer.as_mut_ptr(), buffer.len() as u32);
                (read > 0).then(|| String::from_utf16_lossy(&buffer[..read as usize]))
            })
            .collect()
    }
}

fn source() -> Option<String> {
    unsafe {
        let mut window = GetClipboardOwner();
        if window.is_null() {
            window = GetForegroundWindow();
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, &mut pid);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut path = vec![0; 32768];
        let mut length = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
        windows_sys::Win32::Foundation::CloseHandle(process);
        if ok == 0 {
            return None;
        }
        Path::new(&String::from_utf16_lossy(&path[..length as usize]))
            .file_stem()
            .map(|value| value.to_string_lossy().into_owned())
    }
}

fn bitmap_header(width: u32, height: u32, bits: u16) -> Vec<u8> {
    let mut header = Vec::with_capacity(40);
    header.extend(40u32.to_le_bytes());
    header.extend(width.to_le_bytes());
    header.extend(height.to_le_bytes());
    header.extend(1u16.to_le_bytes());
    header.extend(bits.to_le_bytes());
    header.extend([0; 24]);
    header
}

fn dib_to_bmp(dib: Vec<u8>) -> Option<Vec<u8>> {
    let read_u32 = |offset| {
        Some(u32::from_le_bytes(
            dib.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let read_u16 = |offset| {
        Some(u16::from_le_bytes(
            dib.get(offset..offset + 2)?.try_into().ok()?,
        ))
    };
    let size = read_u32(0)?;
    let (bits, colors, masks, entry_size) = if size == 12 {
        (read_u16(10)?, 0, 0, 3)
    } else if size >= 40 {
        let compression = read_u32(16)?;
        (
            read_u16(14)?,
            read_u32(32)?,
            if size == 40 && compression == 3 {
                12
            } else if size == 40 && compression == 6 {
                16
            } else {
                0
            },
            4,
        )
    } else {
        return None;
    };
    let colors = if colors == 0 && bits <= 8 {
        1 << bits
    } else {
        colors
    };
    let pixel_offset = size
        .checked_add(masks)?
        .checked_add(colors.checked_mul(entry_size)?)?;
    if pixel_offset as usize >= dib.len() {
        return None;
    }
    let mut bmp = Vec::with_capacity(dib.len() + 14);
    bmp.extend(b"BM");
    bmp.extend((dib.len() as u32 + 14).to_le_bytes());
    bmp.extend([0; 4]);
    bmp.extend((pixel_offset + 14).to_le_bytes());
    bmp.extend(dib);
    Some(bmp)
}

fn read_bitmap() -> Option<Vec<u8>> {
    unsafe {
        let bitmap = GetClipboardData(BITMAP_FORMAT);
        if bitmap.is_null() {
            return None;
        }
        let mut info: BITMAP = zeroed();
        if GetObjectW(
            bitmap,
            size_of::<BITMAP>() as i32,
            (&mut info as *mut BITMAP).cast(),
        ) == 0
        {
            return None;
        }
        let width = u32::try_from(info.bmWidth).ok()?;
        let height = u32::try_from(info.bmHeight).ok()?;
        let pixels = u64::from(width) * u64::from(height);
        if pixels == 0 || pixels > 120_000_000 {
            return None;
        }
        let mut data: BITMAPINFO = zeroed();
        data.bmiHeader.biSize = 40;
        data.bmiHeader.biWidth = info.bmWidth;
        data.bmiHeader.biHeight = info.bmHeight;
        data.bmiHeader.biPlanes = 1;
        data.bmiHeader.biBitCount = 32;
        data.bmiHeader.biCompression = BI_RGB;
        let mut dib = bitmap_header(width, height, 32);
        dib.resize(40 + pixels as usize * 4, 0);
        let dc = windows_sys::Win32::Graphics::Gdi::GetDC(null_mut());
        let read = GetDIBits(
            dc,
            bitmap,
            0,
            height,
            dib[40..].as_mut_ptr().cast(),
            &mut data,
            DIB_RGB_COLORS,
        );
        windows_sys::Win32::Graphics::Gdi::ReleaseDC(null_mut(), dc);
        if read != height as i32 {
            return None;
        }
        dib_to_bmp(dib)
    }
}

pub fn screen_size() -> (u32, u32) {
    unsafe {
        (
            GetSystemMetrics(SM_CXSCREEN).max(0) as u32,
            GetSystemMetrics(SM_CYSCREEN).max(0) as u32,
        )
    }
}
