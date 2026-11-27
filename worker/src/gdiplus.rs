use anyhow::{Result, ensure};
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    sync::LazyLock,
};
use windows_sys::{
    Win32::{Graphics::GdiPlus as gdi, UI::Shell::SHCreateMemStream},
    core::GUID,
};

const FORMAT_24_BIT_RGB: i32 = 0x0002_1808;
const PNG_ENCODER: GUID = GUID::from_u128(0x557cf406_1a04_11d3_9a73_0000f81ef32e);
const JPEG_ENCODER: GUID = GUID::from_u128(0x557cf401_1a04_11d3_9a73_0000f81ef32e);
const QUALITY: GUID = GUID::from_u128(0x1d5be4b5_fa4a_452d_9cdd_5db35105e7eb);

static STARTUP: LazyLock<gdi::Status> = LazyLock::new(|| unsafe {
    let mut token = 0;
    let input = gdi::GdiplusStartupInput {
        GdiplusVersion: 1,
        DebugEventCallback: 0,
        SuppressBackgroundThread: 0,
        SuppressExternalCodecs: 0,
    };
    // Images may be processed on several threads. Keep GDI+ initialized for the process.
    gdi::GdiplusStartup(&mut token, &input, null_mut())
});

fn check(status: gdi::Status) -> Result<()> {
    ensure!(status == gdi::Ok, "Windows GDI+ error {status}");
    Ok(())
}

// SHCreateMemStream returns an IStream. The first six methods are
// IUnknown::{QueryInterface,AddRef,Release}, ISequentialStream::{Read,Write}, IStream::Seek.
#[repr(C)]
struct StreamMethods {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    read: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, *mut u32) -> i32,
    write: unsafe extern "system" fn(*mut c_void, *const c_void, u32, *mut u32) -> i32,
    seek: unsafe extern "system" fn(*mut c_void, i64, u32, *mut u64) -> i32,
}

struct Stream(*mut c_void);

impl Stream {
    fn new(bytes: &[u8]) -> Result<Self> {
        let len = u32::try_from(bytes.len())?;
        let data = if len == 0 { null() } else { bytes.as_ptr() };
        let ptr = unsafe { SHCreateMemStream(data, len) };
        ensure!(!ptr.is_null(), "Cannot allocate an image stream");
        Ok(Self(ptr))
    }

    fn methods(&self) -> &StreamMethods {
        unsafe { &**self.0.cast::<*const StreamMethods>() }
    }

    fn read_all(&self) -> Result<Vec<u8>> {
        let vtable = self.methods();
        let mut len = 0u64;
        unsafe {
            ensure!(
                (vtable.seek)(self.0, 0, 2, &mut len) >= 0,
                "Cannot read the image stream size"
            );
            ensure!(
                (vtable.seek)(self.0, 0, 0, null_mut()) >= 0,
                "Cannot rewind the image stream"
            );
            let mut bytes = vec![0; u32::try_from(len)? as usize];
            let mut count = 0;
            ensure!(
                (vtable.read)(
                    self.0,
                    bytes.as_mut_ptr().cast(),
                    bytes.len() as u32,
                    &mut count
                ) >= 0
                    && u64::from(count) == len,
                "Incomplete image stream"
            );
            Ok(bytes)
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        unsafe {
            (self.methods().release)(self.0);
        }
    }
}

pub struct Image {
    ptr: *mut gdi::GpImage,
    _stream: Option<Stream>,
}

struct Graphics(*mut gdi::GpGraphics);

impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            gdi::GdipDeleteGraphics(self.0);
        }
    }
}

impl Image {
    pub fn open(bytes: &[u8]) -> Result<Self> {
        check(*STARTUP)?;
        let stream = Stream::new(bytes)?;
        let mut ptr = null_mut();
        unsafe {
            check(gdi::GdipLoadImageFromStream(stream.0, &mut ptr))?;
        }
        let image = Self {
            ptr,
            _stream: Some(stream),
        };
        let (width, height) = image.dimensions()?;
        ensure!(
            width > 0 && height > 0 && u64::from(width) * u64::from(height) <= 120_000_000,
            "Недопустимый размер изображения."
        );
        Ok(image)
    }

    fn bitmap(width: i32, height: i32) -> Result<Self> {
        let mut bitmap = null_mut();
        unsafe {
            check(gdi::GdipCreateBitmapFromScan0(
                width,
                height,
                0,
                FORMAT_24_BIT_RGB,
                null(),
                &mut bitmap,
            ))?;
        }
        Ok(Self {
            ptr: bitmap.cast(),
            _stream: None,
        })
    }

    pub fn dimensions(&self) -> Result<(u32, u32)> {
        let (mut width, mut height) = (0, 0);
        unsafe {
            check(gdi::GdipGetImageWidth(self.ptr, &mut width))?;
            check(gdi::GdipGetImageHeight(self.ptr, &mut height))?;
        }
        Ok((width, height))
    }

    pub fn png(&self) -> Result<Vec<u8>> {
        self.save(&PNG_ENCODER, null())
    }

    pub fn jpeg(&self, max_width: u32, max_height: u32, quality: u8) -> Result<Vec<u8>> {
        let (width, height) = self.dimensions()?;
        let scale = (max_width as f64 / width as f64)
            .min(max_height as f64 / height as f64)
            .min(1.0);
        let width = (width as f64 * scale).round_ties_even().max(1.0) as i32;
        let height = (height as f64 * scale).round_ties_even().max(1.0) as i32;
        let canvas = self.render(width, height, true)?;
        let mut quality = u32::from(quality.clamp(40, 100));
        let params = gdi::EncoderParameters {
            Count: 1,
            Parameter: [gdi::EncoderParameter {
                Guid: QUALITY,
                NumberOfValues: 1,
                Type: gdi::EncoderParameterValueTypeLong as u32,
                Value: (&mut quality as *mut u32).cast(),
            }],
        };
        canvas.save(&JPEG_ENCODER, &params)
    }

    pub fn fingerprint(&self) -> Result<Vec<u8>> {
        let canvas = self.render(16, 16, false)?;
        let mut pixels = Vec::with_capacity(16 * 16);
        for y in 0..16 {
            for x in 0..16 {
                let mut color = 0;
                unsafe {
                    check(gdi::GdipBitmapGetPixel(canvas.ptr.cast(), x, y, &mut color))?;
                }
                let [_, r, g, b] = color.to_be_bytes().map(u32::from);
                pixels.push(((r * 299 + g * 587 + b * 114) / 1000) as u8);
            }
        }
        Ok(pixels)
    }

    fn render(&self, width: i32, height: i32, thumbnail: bool) -> Result<Self> {
        let canvas = Self::bitmap(width, height)?;
        let mut ptr = null_mut();
        unsafe {
            check(gdi::GdipGetImageGraphicsContext(canvas.ptr, &mut ptr))?;
            let graphics = Graphics(ptr);
            check(gdi::GdipSetInterpolationMode(
                graphics.0,
                if thumbnail {
                    gdi::InterpolationModeHighQualityBicubic
                } else {
                    gdi::InterpolationModeHighQualityBilinear
                },
            ))?;
            check(gdi::GdipSetPixelOffsetMode(
                graphics.0,
                gdi::PixelOffsetModeHighQuality,
            ))?;
            if thumbnail {
                check(gdi::GdipGraphicsClear(graphics.0, 0xff0f_1219))?;
                check(gdi::GdipSetSmoothingMode(
                    graphics.0,
                    gdi::SmoothingModeHighQuality,
                ))?;
            }
            check(gdi::GdipDrawImageRectI(
                graphics.0, self.ptr, 0, 0, width, height,
            ))?;
        }
        Ok(canvas)
    }

    fn save(&self, encoder: &GUID, params: *const gdi::EncoderParameters) -> Result<Vec<u8>> {
        let stream = Stream::new(&[])?;
        unsafe {
            check(gdi::GdipSaveImageToStream(
                self.ptr, stream.0, encoder, params,
            ))?;
        }
        stream.read_all()
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            gdi::GdipDisposeImage(self.ptr);
        }
    }
}
