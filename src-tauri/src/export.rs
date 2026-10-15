//! Writes finished images to a project's output folder: a square PNG master per size, plus
//! optional WebP copies in `<size>/` subfolders. Files are written to a temp name and renamed,
//! so a crash never leaves a half-written image where a game will load it.

use crate::error::{msg, AppResult};
use crate::store::{Item, Project, Store};
use image::{imageops::FilterType, DynamicImage, RgbaImage};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct ExportReport {
    pub written: usize,
    pub skipped: usize,
    pub dir: String,
}

pub fn file_stem(project: &Project, item: &Item) -> String {
    let stem = project
        .filename_pattern
        .replace("{slug}", &item.slug)
        .replace("{n}", &format!("{:03}", item.ord + 1))
        .replace("{project}", &crate::store::slugify(&project.name));
    stem.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '\0')).collect()
}

/// Pads to a square on transparent (centred), then resizes with Lanczos.
pub fn square(img: &DynamicImage, size: u32) -> RgbaImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let side = w.max(h);
    let mut canvas = RgbaImage::from_pixel(side, side, image::Rgba([0, 0, 0, 0]));
    image::imageops::overlay(&mut canvas, &rgba, ((side - w) / 2) as i64, ((side - h) / 2) as i64);
    if side == size {
        canvas
    } else {
        image::imageops::resize(&canvas, size, size, FilterType::Lanczos3)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("eitri-tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn encode_png(img: &RgbaImage) -> AppResult<Vec<u8>> {
    let mut out = Vec::new();
    DynamicImage::ImageRgba8(img.clone()).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

pub fn encode_webp(img: &RgbaImage, quality: f32) -> Vec<u8> {
    let (w, h) = img.dimensions();
    webp::Encoder::from_rgba(img.as_raw(), w, h).encode(quality).to_vec()
}

/// `only_approved`: export approved images only; otherwise every image that has a passing
/// or flagged result (rejected and failed images are always skipped).
pub fn export(store: &Store, project: &Project, dir: &Path, only_approved: bool) -> AppResult<ExportReport> {
    if project.export_sizes.is_empty() && project.webp_sizes.is_empty() {
        return Err(msg("Pick at least one export size."));
    }
    let mut written = 0;
    let mut skipped = 0;
    for item in store.items(&project.id)? {
        let eligible = if only_approved {
            item.status == "approved"
        } else {
            matches!(item.status.as_str(), "approved" | "done" | "flagged")
        };
        let Some(file) = item.file.clone().filter(|_| eligible) else {
            skipped += 1;
            continue;
        };
        let src = image::open(PathBuf::from(&file))?;
        let stem = file_stem(project, &item);
        let multi = project.export_sizes.len() > 1;
        for &size in &project.export_sizes {
            let target = if multi { dir.join(size.to_string()).join(format!("{stem}.png")) } else { dir.join(format!("{stem}.png")) };
            write_atomic(&target, &encode_png(&square(&src, size))?)?;
        }
        for &size in &project.webp_sizes {
            let target = dir.join(size.to_string()).join(format!("{stem}.webp"));
            write_atomic(&target, &encode_webp(&square(&src, size), 82.0))?;
        }
        written += 1;
    }
    store.event(
        Some(&project.id),
        None,
        "export",
        &format!("Exported {written} images to {}", dir.display()),
        None,
    )?;
    Ok(ExportReport { written, skipped, dir: dir.display().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_pads_and_resizes() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(200, 100, image::Rgba([255, 0, 0, 255])));
        let out = square(&img, 64);
        assert_eq!(out.dimensions(), (64, 64));
        assert_eq!(out.get_pixel(32, 2)[3], 0, "padding is transparent");
        assert_eq!(out.get_pixel(32, 32)[3], 255);
    }

    #[test]
    fn webp_keeps_alpha() {
        let mut img = RgbaImage::from_pixel(32, 32, image::Rgba([0, 0, 0, 0]));
        img.put_pixel(16, 16, image::Rgba([10, 200, 10, 255]));
        let bytes = encode_webp(&img, 82.0);
        let back = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(back.get_pixel(0, 0)[3], 0);
    }
}
