use crate::gdiplus;
use anyhow::{Result, ensure};
use image::{
    DynamicImage, ImageFormat, ImageReader, Rgb, codecs::jpeg::JpegEncoder, imageops::FilterType,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::Path};

pub const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff"];

pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

pub fn extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}

pub fn is_image(path: &Path) -> bool {
    EXTENSIONS.contains(&extension(path).as_str())
}

pub enum Picture {
    Windows(gdiplus::Image),
    Raster(DynamicImage),
}

impl Picture {
    pub fn open(bytes: &[u8]) -> Result<Self> {
        if let Ok(image) = gdiplus::Image::open(bytes) {
            return Ok(Self::Windows(image));
        }
        Ok(Self::Raster(decode_raster(bytes)?))
    }

    pub fn dimensions(&self) -> Result<(u32, u32)> {
        match self {
            Self::Windows(image) => image.dimensions(),
            Self::Raster(image) => Ok((image.width(), image.height())),
        }
    }

    pub fn png(&self) -> Result<Vec<u8>> {
        match self {
            Self::Windows(image) => image.png(),
            Self::Raster(image) => png(image),
        }
    }

    pub fn jpeg(&self, width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
        match self {
            Self::Windows(image) => image.jpeg(width, height, quality),
            Self::Raster(image) => raster_jpeg(image, width, height, quality),
        }
    }
}

pub fn decode_raster(bytes: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let (width, height) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()?;
    ensure!(
        u64::from(width) * u64::from(height) <= 120_000_000,
        "Изображение слишком большое."
    );
    reader.limits(image::Limits::default());
    Ok(reader.decode()?)
}

pub fn png(image: &DynamicImage) -> Result<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    image.write_to(&mut buffer, ImageFormat::Png)?;
    Ok(buffer.into_inner())
}

pub fn file_png(path: &Path) -> Result<(Vec<u8>, u32, u32)> {
    ensure!(
        fs::metadata(path)?.len() <= 96 * 1024 * 1024,
        "Изображение слишком большое."
    );
    let image = Picture::open(&fs::read(path)?)?;
    let (width, height) = image.dimensions()?;
    Ok((image.png()?, width, height))
}

fn raster_jpeg(
    image: &DynamicImage,
    max_width: u32,
    max_height: u32,
    quality: u8,
) -> Result<Vec<u8>> {
    let scale = (max_width as f64 / image.width() as f64)
        .min(max_height as f64 / image.height() as f64)
        .min(1.0);
    let width = (image.width() as f64 * scale).round_ties_even().max(1.0) as u32;
    let height = (image.height() as f64 * scale).round_ties_even().max(1.0) as u32;
    let image = image
        .resize_exact(width, height, FilterType::CatmullRom)
        .to_rgba8();
    let rgb = image::RgbImage::from_fn(width, height, |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        let blend = |foreground: u8, background: u8| {
            ((u16::from(foreground) * u16::from(a) + u16::from(background) * (255 - u16::from(a)))
                / 255) as u8
        };
        Rgb([blend(r, 15), blend(g, 18), blend(b, 25)])
    });
    let mut result = Vec::new();
    JpegEncoder::new_with_quality(&mut result, quality).encode_image(&rgb)?;
    Ok(result)
}

pub fn fingerprint(bytes: &[u8]) -> Result<Vec<u8>> {
    if let Ok(image) = gdiplus::Image::open(bytes) {
        return image.fingerprint();
    }
    let image = decode_raster(bytes)?
        .resize_exact(16, 16, FilterType::Triangle)
        .to_rgb8();
    Ok(image
        .pixels()
        .map(|p| {
            let [r, g, b] = p.0.map(u32::from);
            ((r * 299 + g * 587 + b * 114) / 1000) as u8
        })
        .collect())
}

pub fn mime(path: &Path) -> &'static str {
    match extension(path).as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        _ => "image/png",
    }
}
