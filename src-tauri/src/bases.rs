//! The base library: reusable starting points (a style brief, the shared prompt every image ends
//! with, and canvas defaults) kept app-wide and copied into projects. Built-in starters ship with
//! the app; they are read-only and can be duplicated.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Base {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_brief: String,
    pub shared_tail: String,
    pub size: String,
    pub background: String,
    pub gate: String,
    pub builtin: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Filled by queries: style reference image paths.
    #[serde(default)]
    pub references: Vec<BaseRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseRef {
    pub id: String,
    pub file: String,
}

/// Fields the UI may change on a user base. `None` leaves a field as it is.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct BasePatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub style_brief: Option<String>,
    pub shared_tail: Option<String>,
    pub size: Option<String>,
    pub background: Option<String>,
    pub gate: Option<String>,
}

/// Portable file format for sharing a base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseFile {
    pub format: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub style_brief: String,
    pub shared_tail: String,
    #[serde(default = "default_size")]
    pub size: String,
    #[serde(default = "default_background")]
    pub background: String,
    #[serde(default = "default_gate")]
    pub gate: String,
}

pub const FORMAT: &str = "eitri-base/1";

fn default_size() -> String {
    "1024x1024".into()
}
fn default_background() -> String {
    "transparent".into()
}
fn default_gate() -> String {
    "figure".into()
}

const NEGATIVES: &str = "No text, no letters, no numbers, no logos, no watermarks, no signatures. \
This must be an original design and must not resemble any real person, brand, or existing copyrighted character.";

const TRANSPARENT: &str = "Background: fully transparent. Export as a PNG with an alpha channel; every pixel that is not part of \
the subject must be transparent. No white fill, no backdrop, no ground plane, no drop shadow, no glow. The subject itself is fully \
opaque, with no see-through patches, and light colours use off-white (#E8E8E8) rather than pure white.";

pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub style_brief: &'static str,
    pub framing: &'static str,
    pub style: &'static str,
    pub size: &'static str,
    pub background: &'static str,
    pub gate: &'static str,
}

/// Collapses the indentation inside the multi-line literals below into single spaces.
pub fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Builtin {
    pub fn tail(&self) -> String {
        let (w, h) = crate::providers::dims(self.size);
        let bg = if self.background == "transparent" {
            TRANSPARENT.to_string()
        } else {
            "Background: a simple, softly lit backdrop that suits the style and keeps the subject readable.".to_string()
        };
        format!(
            "Framing: {}\nStyle: {}\nCanvas: {w} by {h} pixels.\n{}\n{}",
            clean(self.framing),
            clean(self.style),
            clean(&bg),
            clean(NEGATIVES)
        )
    }
}

pub fn builtins() -> Vec<Builtin> {
    vec![
        Builtin {
            id: "builtin-portrait-bust",
            name: "Cel-shaded portrait bust",
            description: "Head-and-shoulders character portraits for rosters, cards and profiles.",
            style_brief: "Adult-animation cel shading: flat fills with a single hard shadow tone, crisp even ink outlines, \
                          straight-on busts that fill a square frame. Built for hundreds of distinct faces that read as one cast.",
            framing: "head-and-shoulders bust, straight-on frontal view, eyes on the upper third line, the head filling about sixty \
                      percent of a square frame, top of the hair nearly touching the top edge, shoulders cropped by the left and right \
                      edges, collar visible along the bottom edge. The clothing stays one solid shape across the bottom edge.",
            style: "cel-shaded adult-animation portrait, retro mid-century cartoon idiom, flat solid fills with exactly one \
                    hard-edged shadow tone, crisp uniform medium-weight ink outlines, adult proportions, never chibi, never anime, \
                    never photorealistic.",
            size: "1024x1024",
            background: "transparent",
            gate: "portrait",
        },
        Builtin {
            id: "builtin-mascot-crest",
            name: "Mascot crest",
            description: "Team, club or guild emblems built around a mascot.",
            style_brief: "Bold flat-vector sports emblems: a mascot inside a shield or roundel, two or three team colours, thick \
                          outlines, readable at 32 pixels.",
            framing: "a single centred emblem, a mascot head or figure inside a shield, roundel or badge shape, with clear empty space \
                      on every side; the emblem touches no edge.",
            style: "flat vector sports logo, bold geometric shapes, thick dark outline around the whole badge, two or three solid team \
                    colours plus white and black, no gradients, no textures, no photographic detail, crisp at small sizes.",
            size: "1024x1024",
            background: "transparent",
            gate: "figure",
        },
        Builtin {
            id: "builtin-item-icon",
            name: "Game item icon",
            description: "Inventory items, pickups, consumables and loot.",
            style_brief: "Chunky hand-painted game icons: one object per image at a slight three-quarter angle, saturated colour, \
                          soft painted highlights and a dark outline so it pops on any UI panel.",
            framing: "one object centred at a slight three-quarter angle, filling about seventy percent of a square frame, with clear \
                      empty space on every side; the object touches no edge.",
            style: "hand-painted stylised game icon, chunky readable silhouette, saturated colours, soft painted highlights and a \
                    single core shadow, a clean dark outline, consistent top-left lighting, no background scene, no hands holding it.",
            size: "1024x1024",
            background: "transparent",
            gate: "figure",
        },
        Builtin {
            id: "builtin-pixel-sprite",
            name: "Pixel-art sprite",
            description: "Retro characters, enemies and props in a 16-bit look.",
            style_brief: "16-bit era pixel art: a limited palette, hard pixel edges, one-pixel dark outlines and simple dithered shading, \
                          drawn large and crisp.",
            framing: "a single sprite centred in the frame, full body, facing three-quarters toward the viewer, with clear empty space \
                      on every side; the sprite touches no edge.",
            style: "16-bit pixel art, visible square pixels on a coarse grid, limited palette of about sixteen colours, one-pixel dark \
                    outline, simple dithered shading, no anti-aliasing, no blur, no gradients.",
            size: "1024x1024",
            background: "transparent",
            gate: "figure",
        },
        Builtin {
            id: "builtin-isometric-tile",
            name: "Isometric tile",
            description: "Buildings, terrain blocks and props for isometric maps.",
            style_brief: "Clean isometric game tiles: a true 2:1 isometric angle, soft ambient-occlusion shading, gentle bevelled edges \
                          and a bright, friendly palette that tiles together.",
            framing: "one isometric object or terrain block centred in the frame at a true 2:1 isometric angle, the base diamond \
                      sitting in the lower half, with clear empty space on every side; nothing touches an edge.",
            style: "clean isometric 3D-rendered game art, soft ambient occlusion, gentle bevelled edges, consistent light from the top \
                    left, bright friendly palette, no perspective distortion, no ground beyond the tile itself.",
            size: "1024x1024",
            background: "transparent",
            gate: "figure",
        },
        Builtin {
            id: "builtin-character-full",
            name: "Character full-body",
            description: "Full-body characters for selection screens and dialogue.",
            style_brief: "Stylised full-body characters in a confident standing pose, clean shapes, flat colour with soft shading, \
                          proportions consistent across the whole cast.",
            framing: "one full-body character standing in a confident three-quarter pose, head to feet visible, centred and filling \
                      about eighty-five percent of the frame height, with clear space above the head and below the feet.",
            style: "stylised 2D character illustration, clean confident linework, flat colour with soft cel shading, consistent \
                    heroic proportions about seven heads tall, readable silhouette, no scenery.",
            size: "1024x1536",
            background: "transparent",
            gate: "figure",
        },
    ]
}

pub fn to_file(b: &Base) -> BaseFile {
    BaseFile {
        format: FORMAT.into(),
        name: b.name.clone(),
        description: b.description.clone(),
        style_brief: b.style_brief.clone(),
        shared_tail: b.shared_tail.clone(),
        size: b.size.clone(),
        background: b.background.clone(),
        gate: b.gate.clone(),
    }
}

pub fn parse_file(text: &str) -> crate::error::AppResult<BaseFile> {
    let f: BaseFile = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| crate::error::msg(format!("Not a valid base file: {e}")))?;
    if f.shared_tail.trim().is_empty() {
        return Err(crate::error::msg("The base has no shared prompt."));
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_complete_and_unique() {
        let all = builtins();
        assert_eq!(all.len(), 6);
        let mut ids: Vec<_> = all.iter().map(|b| b.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 6);
        for b in &all {
            let t = b.tail();
            assert!(t.starts_with("Framing:"), "{}", b.id);
            assert!(t.contains("Canvas:") && t.contains("No text"), "{}", b.id);
            assert!(!t.contains("  "), "no doubled spaces in {}", b.id);
        }
    }

    #[test]
    fn file_round_trip_and_validation() {
        let f =
            parse_file(r#"{"format":"eitri-base/1","name":"X","shared_tail":"Framing: centred"}"#)
                .unwrap();
        assert_eq!(
            (f.size.as_str(), f.background.as_str(), f.gate.as_str()),
            ("1024x1024", "transparent", "figure")
        );
        assert!(parse_file(r#"{"format":"eitri-base/1","name":"X","shared_tail":" "}"#).is_err());
    }
}
