//! Pixel checks for transparent art. Ported from a 4,100-image production run, where each
//! rule exists because a defect shipped without it.
//!
//! - `Portrait`: a bust cropped by the frame. The top corners must be clear and no
//!   transparency may be sealed inside the chest. The bottom corners are expected to be filled.
//! - `Figure`: a free-standing subject that touches no edge. The background share must be
//!   plausible and nothing may be sealed inside the silhouette.
//! - `None`: opaque art, no checks.
//!
//! Floods run on a 256x256 copy; "open" means alpha below 230.

use image::{imageops::FilterType, DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateMode {
    Portrait,
    Figure,
    None,
}

impl GateMode {
    pub fn parse(s: &str) -> GateMode {
        match s {
            "portrait" => GateMode::Portrait,
            "figure" => GateMode::Figure,
            _ => GateMode::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Pass,
    /// Passed the hard checks but something deserves a look.
    Flag,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateReport {
    pub verdict: Verdict,
    pub reasons: Vec<String>,
    /// Alpha at top-left, top-right, bottom-left, bottom-right.
    pub corners: [u8; 4],
    pub holes: u32,
    pub enclosed: u32,
    pub background: f32,
    pub solid: bool,
    pub width: u32,
    pub height: u32,
}

const N: usize = 256;
const OPEN: u8 = 230;

pub fn evaluate(img: &DynamicImage, mode: GateMode) -> GateReport {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let a = |x: u32, y: u32| rgba.get_pixel(x.min(w - 1), y.min(h - 1))[3];
    let corners = [a(2, 2), a(w.saturating_sub(3), 2), a(2, h.saturating_sub(3)), a(w.saturating_sub(3), h.saturating_sub(3))];
    let small = image::imageops::resize(&rgba, N as u32, N as u32, FilterType::Triangle);
    let open = open_mask(&small);
    let open_count = open.iter().filter(|&&o| o).count();
    let background = open_count as f32 / (N * N) as f32;

    let border: Vec<(usize, usize)> = (0..N)
        .flat_map(|i| [(i, 0), (i, N - 1), (0, i), (N - 1, i)])
        .collect();
    let from_border = flood(&open, &border);
    let enclosed = count_unreached(&open, &from_border, 0..N, 0..N);

    let mut reasons = Vec::new();
    let mut verdict = Verdict::Pass;
    let solid = mode != GateMode::None && corners.iter().all(|&c| c > 200);
    let mut holes = 0;

    match mode {
        GateMode::None => {}
        GateMode::Portrait => {
            let top = flood(&open, &[(0, 0), (N - 1, 0)]);
            holes = count_unreached(&open, &top, 51..205, 184..252);
            if solid {
                verdict = Verdict::Fail;
                reasons.push("Solid background: no transparency at the corners.".into());
            } else if corners[0] != 0 || corners[1] != 0 {
                verdict = Verdict::Fail;
                reasons.push("Top corners are not transparent.".into());
            }
            if holes >= 10 {
                verdict = Verdict::Fail;
                reasons.push(format!("{holes} see-through pixels sealed inside the lower body."));
            }
            if verdict == Verdict::Pass && corners.iter().all(|&c| c < 20) {
                verdict = Verdict::Flag;
                reasons.push("All four corners are clear: the subject may sit small in the frame.".into());
            }
        }
        GateMode::Figure => {
            if solid {
                verdict = Verdict::Fail;
                reasons.push("Solid background: no transparency at the corners.".into());
            } else if !(background > 0.08 && background < 0.88) {
                verdict = Verdict::Fail;
                reasons.push(format!("Background covers {:.0}% of the frame.", background * 100.0));
            }
            if enclosed >= 10 {
                verdict = Verdict::Fail;
                reasons.push(format!("{enclosed} see-through pixels sealed inside the subject."));
            }
        }
    }
    if w != h {
        if verdict == Verdict::Pass {
            verdict = Verdict::Flag;
        }
        reasons.push(format!("Not square: {w}x{h}."));
    }

    GateReport { verdict, reasons, corners, holes, enclosed, background, solid, width: w, height: h }
}

fn open_mask(img: &RgbaImage) -> Vec<bool> {
    img.pixels().map(|p| p[3] < OPEN).collect()
}

fn flood(open: &[bool], seeds: &[(usize, usize)]) -> Vec<bool> {
    let mut seen = vec![false; N * N];
    let mut stack: Vec<(usize, usize)> = seeds.iter().copied().filter(|&(x, y)| open[y * N + x]).collect();
    while let Some((x, y)) = stack.pop() {
        let i = y * N + x;
        if seen[i] || !open[i] {
            continue;
        }
        seen[i] = true;
        if x > 0 { stack.push((x - 1, y)); }
        if x + 1 < N { stack.push((x + 1, y)); }
        if y > 0 { stack.push((x, y - 1)); }
        if y + 1 < N { stack.push((x, y + 1)); }
    }
    seen
}

fn count_unreached(open: &[bool], seen: &[bool], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> u32 {
    let mut n = 0;
    for y in ys {
        for x in xs.clone() {
            let i = y * N + x;
            if open[i] && !seen[i] {
                n += 1;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    const S: u32 = 512;
    const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);
    const INK: Rgba<u8> = Rgba([200, 120, 90, 255]);

    fn canvas(fill: Rgba<u8>) -> RgbaImage {
        RgbaImage::from_pixel(S, S, fill)
    }
    fn rect(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, c: Rgba<u8>) {
        for y in y0..y1 {
            for x in x0..x1 {
                img.put_pixel(x, y, c);
            }
        }
    }
    fn bust() -> RgbaImage {
        // Head in the upper middle, shoulders filling the full width of the lower band.
        let mut img = canvas(CLEAR);
        rect(&mut img, 176, 60, 336, 300, INK);
        rect(&mut img, 0, 300, S, S, INK);
        img
    }

    #[test]
    fn portrait_bust_passes() {
        let r = evaluate(&DynamicImage::ImageRgba8(bust()), GateMode::Portrait);
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.reasons);
    }

    #[test]
    fn portrait_sealed_torso_hole_fails() {
        let mut img = bust();
        rect(&mut img, 220, 400, 290, 460, CLEAR);
        let r = evaluate(&DynamicImage::ImageRgba8(img), GateMode::Portrait);
        assert_eq!(r.verdict, Verdict::Fail);
        assert!(r.holes >= 10);
    }

    #[test]
    fn portrait_solid_background_fails() {
        let r = evaluate(&DynamicImage::ImageRgba8(canvas(Rgba([255, 255, 255, 255]))), GateMode::Portrait);
        assert_eq!(r.verdict, Verdict::Fail);
        assert!(r.solid);
    }

    #[test]
    fn portrait_small_subject_is_flagged() {
        let mut img = canvas(CLEAR);
        rect(&mut img, 200, 200, 312, 400, INK);
        let r = evaluate(&DynamicImage::ImageRgba8(img), GateMode::Portrait);
        assert_eq!(r.verdict, Verdict::Flag);
    }

    #[test]
    fn figure_with_arms_out_passes() {
        // A body with arms reaching out: the gaps under the arms are open to the outside.
        let mut img = canvas(CLEAR);
        rect(&mut img, 216, 80, 296, 440, INK);
        rect(&mut img, 96, 160, 416, 190, INK);
        rect(&mut img, 96, 190, 120, 300, INK);
        rect(&mut img, 392, 190, 416, 300, INK);
        let r = evaluate(&DynamicImage::ImageRgba8(img), GateMode::Figure);
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.reasons);
    }

    #[test]
    fn figure_with_sealed_hole_fails() {
        let mut img = canvas(CLEAR);
        rect(&mut img, 156, 80, 356, 440, INK);
        rect(&mut img, 220, 200, 290, 280, CLEAR);
        let r = evaluate(&DynamicImage::ImageRgba8(img), GateMode::Figure);
        assert_eq!(r.verdict, Verdict::Fail);
        assert!(r.enclosed >= 10);
    }

    #[test]
    fn none_mode_accepts_opaque_art() {
        let r = evaluate(&DynamicImage::ImageRgba8(canvas(Rgba([10, 20, 30, 255]))), GateMode::None);
        assert_eq!(r.verdict, Verdict::Pass);
    }
}
