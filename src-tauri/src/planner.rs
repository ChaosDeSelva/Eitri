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
    /// When the project already has a style (from a base), the planner writes only the items and
    /// this style brief and shared prompt are kept exactly.
    #[serde(default)]
    pub keep_style: Option<KeepStyle>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct KeepStyle {
    pub style_brief: String,
    pub shared_tail: String,
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
    ) + &keep_style_message(req)
}

fn keep_style_message(req: &PlanRequest) -> String {
    match &req.keep_style {
        Some(k) if !k.shared_tail.trim().is_empty() => format!(
            "\n\nThe style is already decided. Return this style_brief and shared_tail exactly as given, and write \
             only the name and the items so every subject fits this style without restating it.\n\nstyle_brief:\n{}\n\nshared_tail:\n{}",
            k.style_brief.trim(),
            k.shared_tail.trim()
        ),
        _ => String::new(),
    }
}

/// Keeps a chosen base's style even if the model paraphrased it.
pub fn enforce_style(mut plan: Plan, req: &PlanRequest) -> Plan {
    if let Some(k) = req
        .keep_style
        .as_ref()
        .filter(|k| !k.shared_tail.trim().is_empty())
    {
        plan.style_brief = k.style_brief.clone();
        plan.shared_tail = k.shared_tail.clone();
    }
    plan
}

// ---------- base generation ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseDraft {
    pub name: String,
    pub description: String,
    pub style_brief: String,
    pub shared_tail: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BaseRequest {
    pub look: String,
    pub size: String,
    pub background: String,
    pub gate: String,
}

pub fn base_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "description", "style_brief", "shared_tail"],
        "properties": {
            "name": {"type": "string", "description": "Short name for the look, 2 to 4 words."},
            "description": {"type": "string", "description": "One sentence: what sets this base is for."},
            "style_brief": {"type": "string", "description": "Two or three sentences a person reads to understand the look."},
            "shared_tail": {"type": "string", "description": "Prompt text appended to every image made with this base."}
        }
    })
}

const BASE_SYSTEM: &str = r#"You write reusable base prompts for an image model. A base is the generic half of every
prompt in a set: it fixes the look, and each image later adds its own specific subject.

Write:
- name: a short name for the look.
- description: one sentence on what kinds of sets it suits.
- style_brief: two or three plain sentences for a person.
- shared_tail: labeled lines, exactly in this order:
  "Framing:" composition, crop and where the subject sits in the frame.
  "Style:" medium, rendering, line work, shading, palette rules, and what to avoid.
  "Canvas:" the pixel size.
  "Background:" as instructed below. If transparent: every pixel outside the subject is transparent, the subject
  is fully opaque with no see-through patches, no drop shadow, glow or ground plane, and light colours use off-white
  rather than pure white.
  Then one line of negatives: no text, letters, numbers, logos or watermarks, no real people, brands or existing
  characters.
Never describe a particular subject in the base; it must work for any subject in the set."#;

pub fn base_user_message(req: &BaseRequest) -> String {
    let probe = PlanRequest {
        idea: String::new(),
        count: 1,
        background: req.background.clone(),
        gate: req.gate.clone(),
        size: req.size.clone(),
        notes: String::new(),
        keep_style: None,
    };
    let lines = user_message(&probe);
    let kept: Vec<&str> = lines
        .lines()
        .filter(|l| {
            l.starts_with("Canvas") || l.starts_with("Background") || l.starts_with("Composition")
        })
        .collect();
    format!("Look: {}\n{}", req.look.trim(), kept.join("\n"))
}

pub async fn draft_base(
    provider: &crate::providers::Provider,
    model: &str,
    req: &BaseRequest,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<(BaseDraft, Usage), ApiError> {
    let (v, usage) = provider
        .plan(
            model,
            BASE_SYSTEM,
            &base_user_message(req),
            &base_schema(),
            on_text,
        )
        .await?;
    let d: BaseDraft = serde_json::from_value(v)
        .map_err(|e| ApiError::Transient(format!("base did not match the schema: {e}")))?;
    if d.shared_tail.trim().is_empty() {
        return Err(ApiError::Transient("the base came back empty".into()));
    }
    Ok((d, usage))
}

pub fn parse(v: Value) -> Result<Plan, ApiError> {
    let plan: Plan = serde_json::from_value(v)
        .map_err(|e| ApiError::Transient(format!("plan did not match the schema: {e}")))?;
    if plan.items.is_empty() {
        return Err(ApiError::Transient("the plan had no items".into()));
    }
    Ok(plan)
}

pub async fn draft(
    client: &OpenAi,
    model: &str,
    req: &PlanRequest,
) -> Result<(Plan, Usage), ApiError> {
    let (v, usage) = client
        .respond_json(model, SYSTEM, &user_message(req), "image_spec", &schema())
        .await?;
    Ok((parse(v)?, usage))
}

/// Streaming draft through any planning-capable provider: `on_text` gets the partial JSON as
/// it is written.
pub async fn draft_streaming(
    provider: &crate::providers::Provider,
    model: &str,
    req: &PlanRequest,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<(Plan, Usage), ApiError> {
    let (v, usage) = provider
        .plan(model, SYSTEM, &user_message(req), &schema(), on_text)
        .await?;
    Ok((enforce_style(parse(v)?, req), usage))
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
        assert!(
            parse(json!({"name": "x", "style_brief": "", "shared_tail": "", "items": []})).is_err()
        );
    }

    #[test]
    fn kept_style_is_requested_and_enforced() {
        let req = PlanRequest {
            idea: "potions".into(),
            count: 2,
            background: "transparent".into(),
            gate: "figure".into(),
            size: "1024x1024".into(),
            notes: String::new(),
            keep_style: Some(KeepStyle {
                style_brief: "Chunky.".into(),
                shared_tail: "Framing: centred.".into(),
            }),
        };
        assert!(user_message(&req).contains("shared_tail:\nFraming: centred."));
        let plan = Plan {
            name: "x".into(),
            style_brief: "paraphrased".into(),
            shared_tail: "paraphrased".into(),
            items: vec![],
        };
        let kept = enforce_style(plan, &req);
        assert_eq!(kept.style_brief, "Chunky.");
        assert_eq!(kept.shared_tail, "Framing: centred.");
    }

    #[test]
    fn base_request_carries_canvas_and_background() {
        let m = base_user_message(&BaseRequest {
            look: "pixel art".into(),
            size: "1024x1024".into(),
            background: "transparent".into(),
            gate: "figure".into(),
        });
        assert!(m.starts_with("Look: pixel art"));
        assert!(
            m.contains("Canvas: 1024x1024") && m.contains("transparent") && !m.contains("Idea:")
        );
        assert_eq!(base_schema()["additionalProperties"], false);
    }

    #[test]
    fn schema_is_strict() {
        let s = schema();
        assert_eq!(s["additionalProperties"], false);
        assert_eq!(
            s["properties"]["items"]["items"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn user_message_clamps_count() {
        let m = user_message(&PlanRequest {
            idea: "potions".into(),
            count: 9999,
            background: "transparent".into(),
            gate: "figure".into(),
            size: "1024x1024".into(),
            notes: String::new(),
            keep_style: None,
        });
        assert!(m.contains("Number of items: 500"));
        assert!(m.contains("transparent"));
    }
}
