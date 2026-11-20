//! Decode bounded project images outside the rendering thread.

use crate::domain::Failure;
use image::{ImageFormat, ImageReader};
use resvg::{tiny_skia, usvg};
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

const MAX_INPUT: u64 = 16 * 1024 * 1024;
const MAX_PIXELS: u64 = 24_000_000;
const PREVIEW_EDGE: u32 = 2048;
static DECODERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

pub(crate) fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            ["png", "jpg", "jpeg", "webp", "gif", "bmp", "ico", "svg"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

pub(crate) async fn read(root: PathBuf, path: PathBuf) -> Result<image::RgbaImage, Failure> {
    let permit = DECODERS.acquire().await.map_err(|_| Failure::Unavailable)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let resolved = path.canonicalize().map_err(|_| Failure::Read)?;
        if !resolved.starts_with(root.canonicalize().map_err(|_| Failure::Read)?) {
            return Err(Failure::Read);
        }
        let metadata = resolved.metadata().map_err(|_| Failure::Read)?;
        if !metadata.is_file() {
            return Err(Failure::Read);
        }
        if metadata.len() > MAX_INPUT {
            return Err(Failure::TooLarge);
        }
        let file = std::fs::File::open(resolved).map_err(|_| Failure::Read)?;
        let mut bytes = Vec::new();
        file.take(MAX_INPUT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Failure::Read)?;
        if bytes.len() as u64 > MAX_INPUT {
            return Err(Failure::TooLarge);
        }
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            return svg(&bytes);
        }
        let format = ImageFormat::from_path(path).map_err(|_| Failure::Read)?;
        let reader = || ImageReader::with_format(Cursor::new(&bytes), format);
        let (width, height) = reader().into_dimensions().map_err(|_| Failure::Read)?;
        if u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Err(Failure::TooLarge);
        }
        let mut decoder = reader();
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        decoder.limits(limits);
        let image = decoder.decode().map_err(|err| match err {
            image::ImageError::Limits(_) => Failure::TooLarge,
            _ => Failure::Read,
        })?;
        let image = if width.max(height) > PREVIEW_EDGE {
            image.thumbnail(PREVIEW_EDGE, PREVIEW_EDGE)
        } else {
            image
        };
        Ok(image.into_rgba8())
    })
    .await
    .map_err(|_| Failure::Read)?
}

fn svg(bytes: &[u8]) -> Result<image::RgbaImage, Failure> {
    static FONTS: LazyLock<Arc<usvg::fontdb::Database>> = LazyLock::new(|| {
        let mut fonts = usvg::fontdb::Database::new();
        fonts.load_system_fonts();
        Arc::new(fonts)
    });
    let embedded_pixels = std::sync::atomic::AtomicU64::new(0);
    let options = usvg::Options {
        fontdb: Arc::clone(&FONTS),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            resolve_data: Box::new(|_, bytes, _| {
                use std::sync::atomic::Ordering;
                let reader = ImageReader::new(Cursor::new(bytes.as_slice()))
                    .with_guessed_format()
                    .ok()?;
                let format = reader.format()?;
                let (width, height) = reader.into_dimensions().ok()?;
                let count = u64::from(width) * u64::from(height);
                embedded_pixels
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                        used.checked_add(count).filter(|total| *total <= MAX_PIXELS)
                    })
                    .ok()?;
                match format {
                    ImageFormat::Png => Some(usvg::ImageKind::PNG(bytes)),
                    ImageFormat::Jpeg => Some(usvg::ImageKind::JPEG(bytes)),
                    ImageFormat::Gif => Some(usvg::ImageKind::GIF(bytes)),
                    ImageFormat::WebP => Some(usvg::ImageKind::WEBP(bytes)),
                    _ => None,
                }
            }),
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(
        std::str::from_utf8(bytes).map_err(|_| Failure::Read)?,
        &options,
    )
    .map_err(|_| Failure::Read)?;
    let size = tree.size();
    let scale = (PREVIEW_EDGE as f32 / size.width().max(size.height())).min(1.);
    let width = (size.width() * scale).round().max(1.) as u32;
    let height = (size.height() * scale).round().max(1.) as u32;
    let mut pixels = tiny_skia::Pixmap::new(width, height).ok_or(Failure::TooLarge)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixels.as_mut(),
    );
    let rgba = pixels
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(width, height, rgba).ok_or(Failure::Read)
}

#[cfg(test)]
mod tests;
