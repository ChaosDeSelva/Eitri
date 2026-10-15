//! Background keying for providers that cannot return transparency. The prompt asks for a flat
//! chroma-green backdrop; this removes it by flooding from the frame edge, so only background
//! that touches the border is cleared and green inside the subject survives. Edge pixels are
//! softened and de-spilled so the outline does not keep a green fringe.

use image::{Rgba, RgbaImage};

pub const KEY_TAIL: &str = "Background: a perfectly flat, uniform chroma-key green (#00FF00) backdrop filling every pixel \
around the subject, with no texture, gradient, vignette, shadow, glow, reflection or ground plane. Do not use that green \
anywhere on the subject itself.";

const HARD: f32 = 70.0;
const SOFT: f32 = 140.0;

fn dist(p: &Rgba<u8>, k: [f32; 3]) -> f32 {
    let d = [p[0] as f32 - k[0], p[1] as f32 - k[1], p[2] as f32 - k[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The backdrop colour: the median of the border pixels, so a stray subject pixel at the edge
/// does not throw it off.
fn border_color(img: &RgbaImage) -> ([f32; 3], f32) {
    let (w, h) = img.dimensions();
    let mut px = Vec::new();
    for x in (0..w).step_by(4) {
        px.push(*img.get_pixel(x, 0));
        px.push(*img.get_pixel(x, h - 1));
    }
    for y in (0..h).step_by(4) {
        px.push(*img.get_pixel(0, y));
        px.push(*img.get_pixel(w - 1, y));
    }
    let med = |c: usize| {
        let mut v: Vec<u8> = px.iter().map(|p| p[c]).collect();
        v.sort_unstable();
        v[v.len() / 2] as f32
    };
    let key = [med(0), med(1), med(2)];
    let share = px.iter().filter(|p| dist(p, key) < HARD).count() as f32 / px.len() as f32;
    (key, share)
}

/// Returns the keyed PNG, or `None` when the border is not a flat backdrop (most of the edge
/// must match), in which case the image is left alone and the pixel checks decide.
pub fn key_out(png: &[u8]) -> Option<Vec<u8>> {
    let mut img = image::load_from_memory(png).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return None;
    }
    let (key, share) = border_color(&img);
    if share < 0.8 {
        return None;
    }
    let idx = |x: u32, y: u32| (y * w + x) as usize;
    let mut bg = vec![false; (w * h) as usize];
    let mut stack: Vec<(u32, u32)> = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let i = idx(x, y);
        if bg[i] || dist(img.get_pixel(x, y), key) >= HARD {
            continue;
        }
        bg[i] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let green_key = key[1] > key[0] + 40.0 && key[1] > key[2] + 40.0;
    let snapshot = img.clone();
    for y in 0..h {
        for x in 0..w {
            let i = idx(x, y);
            if bg[i] {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }
            // Pixels on the subject's outline: fade by closeness to the key and remove spill.
            let touches = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0
                        && ny >= 0
                        && (nx as u32) < w
                        && (ny as u32) < h
                        && bg[idx(nx as u32, ny as u32)]
                });
            if !touches {
                continue;
            }
            let mut p = *snapshot.get_pixel(x, y);
            let d = dist(&p, key);
            if d < SOFT {
                let a = ((d - HARD) / (SOFT - HARD)).clamp(0.2, 1.0);
                p[3] = (p[3] as f32 * a) as u8;
            }
            if green_key {
                let limit = p[0].max(p[2]);
                if p[1] > limit {
                    p[1] = limit;
                }
            }
            img.put_pixel(x, y, p);
        }
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn clears_the_backdrop_and_keeps_inner_green() {
        let green = Rgba([0, 255, 0, 255]);
        let mut img = RgbaImage::from_pixel(64, 64, green);
        for y in 16..48 {
            for x in 16..48 {
                img.put_pixel(x, y, Rgba([180, 60, 40, 255]));
            }
        }
        // A green patch sealed inside the subject must survive.
        for y in 28..36 {
            for x in 28..36 {
                img.put_pixel(x, y, green);
            }
        }
        let out = image::load_from_memory(&key_out(&png(&img)).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(out.get_pixel(2, 2)[3], 0);
        assert_eq!(out.get_pixel(20, 20)[3], 255);
        assert_eq!(out.get_pixel(32, 32)[3], 255);
    }

    #[test]
    fn leaves_busy_backgrounds_alone() {
        let mut img = RgbaImage::new(64, 64);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgba([(x * 4) as u8, (y * 4) as u8, 128, 255]);
        }
        assert!(key_out(&png(&img)).is_none());
    }
}
