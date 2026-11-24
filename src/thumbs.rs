//! Test pictures from `images_dir`, shrunk to thumbnails in background threads.
//! Only the thumbnails stay in memory; quick view decodes one picture again at screen size.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use slint::{Rgba8Pixel, SharedPixelBuffer};

pub type Pixels = SharedPixelBuffer<Rgba8Pixel>;

const EXTENSIONS: [&str; 8] = ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff"];
const MAX_PICTURES: usize = 60;
/// Long side of a thumbnail in pixels: enough for the largest preview size without upscaling much.
pub const THUMB_SIDE: u32 = 480;
const THREADS: usize = 2;

pub fn list(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| EXTENSIONS.contains(&extension.to_lowercase().as_str()))
        })
        .collect();
    paths.sort();
    paths.truncate(MAX_PICTURES);
    paths
}

/// Decodes every picture and hands over (index, original width, original height, thumbnail).
pub fn spawn_loading(paths: Vec<PathBuf>, on_loaded: impl Fn(usize, u32, u32, Pixels) + Send + Sync + 'static) {
    let queue = Arc::new(Mutex::new(paths.into_iter().enumerate().collect::<Vec<_>>()));
    let on_loaded = Arc::new(on_loaded);
    for _ in 0..THREADS {
        let queue = queue.clone();
        let on_loaded = on_loaded.clone();
        std::thread::spawn(move || {
            loop {
                let Some((index, path)) = queue.lock().ok().and_then(|mut queue| queue.pop()) else {
                    return;
                };
                if let Some(picture) = decode(&path) {
                    let (width, height) = (picture.width(), picture.height());
                    on_loaded(index, width, height, fit(&picture, THUMB_SIDE));
                }
            }
        });
    }
}

/// One picture at up to `side` pixels on the long side, for quick view.
pub fn spawn_full(path: PathBuf, side: u32, on_loaded: impl FnOnce(Pixels) + Send + 'static) {
    std::thread::spawn(move || {
        if let Some(picture) = decode(&path) {
            on_loaded(fit(&picture, side));
        }
    });
}

fn decode(path: &Path) -> Option<RgbaImage> {
    let mut decoder = ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_decoder().ok()?;
    let orientation = decoder.orientation().ok()?;
    let mut picture = DynamicImage::from_decoder(decoder).ok()?;
    picture.apply_orientation(orientation);
    Some(picture.into_rgba8())
}

fn fit(source: &RgbaImage, side: u32) -> Pixels {
    let (width, height) = (source.width(), source.height());
    let scale = (side as f32 / width.max(height) as f32).min(1.0);
    let target = (((width as f32 * scale).round() as u32).max(1), ((height as f32 * scale).round() as u32).max(1));
    let unchanged = || Pixels::clone_from_slice(source.as_raw(), width, height);
    if target == (width, height) {
        return unchanged();
    }
    let Ok(src) = ImageRef::new(width, height, source.as_raw(), PixelType::U8x4) else {
        return unchanged();
    };
    let mut dst = Image::new(target.0, target.1, PixelType::U8x4);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));
    if Resizer::new().resize(&src, &mut dst, &options).is_err() {
        return unchanged();
    }
    Pixels::clone_from_slice(dst.buffer(), target.0, target.1)
}
