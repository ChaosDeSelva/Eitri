//! Turns a one-line idea into a spec: a style brief, a shared prompt tail every image ends
//! with, and a list of distinct items. The shared tail is what keeps a large set coherent;
//! each item's subject is what keeps the images from all looking alike.

use crate::openai::{ApiError, OpenAi, Usage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlannedItem {
    pub slug: String,
    pub title: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub name: String,
    pub style_brief: String,
    pub shared_tail: String,
    pub items: Vec<PlannedItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanRequest {
    pub idea: String,
    pub count: u32,
    pub background: String,
    pub gate: String,
    pub size: String,
    /// Optional extra direction (style, audience, must-haves).
    #[serde(default)]
    pub notes: String,
}

pub fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "style_brief", "shared_tail", "items"],
        "properties": {
            "name": {"type": "string", "description": "Short project name, 2 to 4 words."},
            "style_brief": {"type": "string", "description": "Two or three sentences a person reads to understand the look."},
            "shared_tail": {"type": "string", "description": "Prompt text appended to every item."},
            "items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["slug", "title", "subject"],
                    "properties": {
                        "slug": {"type": "string", "description": "lowercase-kebab-case file name, unique"},
                        "title": {"type": "string"},
                        "subject": {"type": "string", "description": "Full description of this one image."}
                    }
                }
            }
        }
    })
}

const SYSTEM: &str = r#"You plan batches of images for an image model. You write a spec, not images.

Write three things:
1. style_brief: two or three plain sentences describing the shared look, for a person.
2. shared_tail: the prompt text appended to EVERY item. It must fully pin down everything the set
   shares, as labeled lines:
   - "Framing:" exact composition, crop and where the subject sits in a square frame.
   - "Style:" medium, rendering, line weight, shading, palette rules, what to avoid
     (for example "never photorealistic").
   - "Canvas:" the pixel size, square.
   - "Background:" exactly as instructed below.
   - Then negatives: no text, no letters, no numbers, no logos or watermarks, no real people,
     no real brands, no copyrighted characters.
   If the background is transparent, say the subject itself is fully opaque with no see-through
   patches, every pixel outside the subject is transparent, no drop shadow, no glow, no ground
   plane, and light garments use off-white rather than pure white.
3. items: exactly the requested number. Each subject is a complete, specific description of ONE
   image (who or what, distinguishing features, colors, pose or angle, expression or state). Vary
   the items along several independent axes so no two read as the same image. Do not repeat
   anything the shared tail already says. Never describe style in a subject.

Slugs are short lowercase-kebab-case and unique. Never name real people, brands or existing
fictional characters."#;

pub fn user_message(req: &PlanRequest) -> String {
    let background = match req.background.as_str() {
        "transparent" => "transparent (PNG with alpha)",
        "opaque" => "opaque, a simple backdrop that suits the style",
        _ => "whatever suits the style",
    };
    let framing = match req.gate.as_str() {
        "portrait" => "a head-and-shoulders bust cropped by the left, right and bottom edges, top corners empty",
        "figure" => "a free-standing subject centred with clear space on every side, touching no edge",
        _ => "whatever composition suits the idea",
    };
    format!(
        "Idea: {}\nNumber of items: {}\nCanvas: {}\nBackground: {}\nComposition: {}\nExtra direction: {}",
        req.idea.trim(),
        req.count.clamp(1, 500),
        req.size,
        background,
        framing,
        if req.notes.trim().is_empty() { "none" } else { req.notes.trim() }
    )
}

pub fn parse(v: Value) -> Result<Plan, ApiError> {
    let plan: Plan = serde_json::from_value(v).map_err(|e| ApiError::Transient(format!("plan did not match the schema: {e}")))?;
    if plan.items.is_empty() {
        return Err(ApiError::Transient("the plan had no items".into()));
    }
    Ok(plan)
}

pub async fn draft(client: &OpenAi, model: &str, req: &PlanRequest) -> Result<(Plan, Usage), ApiError> {
    let (v, usage) = client.respond_json(model, SYSTEM, &user_message(req), "image_spec", &schema()).await?;
    Ok((parse(v)?, usage))
}

/// Streaming draft: `on_text` gets the partial JSON as it is written.
pub async fn draft_streaming(
    client: &OpenAi,
    model: &str,
    req: &PlanRequest,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<(Plan, Usage), ApiError> {
    let (v, usage) = client
        .respond_json_stream(model, SYSTEM, &user_message(req), "image_spec", &schema(), on_text)
        .await?;
    Ok((parse(v)?, usage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_structured_plan() {
        let v = json!({
            "name": "Potion Icons",
            "style_brief": "Chunky flat icons.",
            "shared_tail": "Framing: centred.",
            "items": [{"slug": "red-potion", "title": "Red", "subject": "A red potion"}]
        });
        let p = parse(v).unwrap();
        assert_eq!(p.items[0].slug, "red-potion");
        assert!(parse(json!({"name": "x", "style_brief": "", "shared_tail": "", "items": []})).is_err());
    }

    #[test]
    fn schema_is_strict() {
        let s = schema();
        assert_eq!(s["additionalProperties"], false);
        assert_eq!(s["properties"]["items"]["items"]["additionalProperties"], false);
    }

    #[test]
    fn user_message_clamps_count() {
        let m = user_message(&PlanRequest {
            idea: "potions".into(), count: 9999, background: "transparent".into(),
            gate: "figure".into(), size: "1024x1024".into(), notes: String::new(),
        });
        assert!(m.contains("Number of items: 500"));
        assert!(m.contains("transparent"));
    }
}
