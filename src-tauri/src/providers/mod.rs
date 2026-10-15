//! Image and planning providers behind one interface.
//!
//! OpenAI is the reference implementation and the one exercised in production. The others are
//! written against each provider's public API documentation; if one misbehaves, the user-facing
//! error says which provider and model failed so it can be reported.

pub mod fal;
pub mod google;
pub mod replicate;
pub mod stability;

use crate::openai::{ApiError, ImageRequest, ImageResult, OpenAi, Usage, DEFAULT_BASE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Openai,
    Google,
    Fal,
    Replicate,
    Stability,
    /// Any server that speaks the OpenAI images API (local models, gateways).
    Compatible,
}

impl Kind {
    pub fn parse(s: &str) -> Kind {
        match s {
            "google" => Kind::Google,
            "fal" => Kind::Fal,
            "replicate" => Kind::Replicate,
            "stability" => Kind::Stability,
            "compatible" => Kind::Compatible,
            _ => Kind::Openai,
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Kind::Openai => "openai",
            Kind::Google => "google",
            Kind::Fal => "fal",
            Kind::Replicate => "replicate",
            Kind::Stability => "stability",
            Kind::Compatible => "compatible",
        }
    }
    pub const ALL: [Kind; 6] = [
        Kind::Openai,
        Kind::Google,
        Kind::Fal,
        Kind::Replicate,
        Kind::Stability,
        Kind::Compatible,
    ];
}

/// What a model can do. Drives both the runner and the hints in the UI.
#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq)]
pub struct Caps {
    /// Returns real alpha. Without it, transparent projects get a flat backdrop that is keyed out.
    pub transparent: bool,
    /// Accepts reference images for style matching.
    pub references: bool,
    /// Can edit a given image (used for redo-with-note).
    pub edit: bool,
    /// Quality setting is honoured.
    pub quality: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub caps: Caps,
    /// Typical USD per square image, for estimates before a run.
    pub estimate_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub key_url: &'static str,
    pub key_hint: &'static str,
    pub needs_key: bool,
    pub needs_base_url: bool,
    pub planning: bool,
    pub image_models: Vec<ModelInfo>,
    pub text_models: Vec<&'static str>,
    pub note: &'static str,
}

const fn caps(transparent: bool, references: bool, edit: bool, quality: bool) -> Caps {
    Caps {
        transparent,
        references,
        edit,
        quality,
    }
}

pub fn catalog() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "openai",
            name: "OpenAI",
            key_url: "https://platform.openai.com/api-keys",
            key_hint: "sk-...",
            needs_key: true,
            needs_base_url: false,
            planning: true,
            image_models: vec![
                ModelInfo { id: "gpt-image-1.5", label: "GPT Image 1.5", caps: caps(true, true, true, true), estimate_usd: 0.06 },
                ModelInfo { id: "gpt-image-1", label: "GPT Image 1", caps: caps(true, true, true, true), estimate_usd: 0.042 },
                ModelInfo { id: "gpt-image-1-mini", label: "GPT Image 1 mini", caps: caps(true, true, true, true), estimate_usd: 0.011 },
            ],
            text_models: vec!["gpt-5-mini", "gpt-4.1-mini", "gpt-4.1", "gpt-4o-mini"],
            note: "Reference provider. Real transparency, style references and edits.",
        },
        ProviderInfo {
            id: "google",
            name: "Google Gemini",
            key_url: "https://aistudio.google.com/apikey",
            key_hint: "AIza...",
            needs_key: true,
            needs_base_url: false,
            planning: true,
            image_models: vec![
                ModelInfo { id: "gemini-2.5-flash-image", label: "Gemini 2.5 Flash Image", caps: caps(false, true, true, false), estimate_usd: 0.039 },
                ModelInfo { id: "imagen-4.0-generate-001", label: "Imagen 4", caps: caps(false, false, false, false), estimate_usd: 0.04 },
                ModelInfo { id: "imagen-4.0-ultra-generate-001", label: "Imagen 4 Ultra", caps: caps(false, false, false, false), estimate_usd: 0.06 },
                ModelInfo { id: "imagen-4.0-fast-generate-001", label: "Imagen 4 Fast", caps: caps(false, false, false, false), estimate_usd: 0.02 },
            ],
            text_models: vec!["gemini-2.5-flash", "gemini-2.5-pro", "gemini-2.5-flash-lite"],
            note: "Gemini image models take style references; Imagen does not. No native transparency.",
        },
        ProviderInfo {
            id: "fal",
            name: "fal.ai",
            key_url: "https://fal.ai/dashboard/keys",
            key_hint: "key-id:key-secret",
            needs_key: true,
            needs_base_url: false,
            planning: false,
            image_models: vec![
                ModelInfo { id: "fal-ai/flux-pro/v1.1", label: "FLUX1.1 [pro]", caps: caps(false, false, false, false), estimate_usd: 0.04 },
                ModelInfo { id: "fal-ai/flux/dev", label: "FLUX.1 [dev]", caps: caps(false, false, false, false), estimate_usd: 0.025 },
                ModelInfo { id: "fal-ai/flux/schnell", label: "FLUX.1 [schnell]", caps: caps(false, false, false, false), estimate_usd: 0.003 },
                ModelInfo { id: "fal-ai/flux-pro/kontext", label: "FLUX.1 Kontext [pro]", caps: caps(false, true, true, false), estimate_usd: 0.04 },
                ModelInfo { id: "fal-ai/recraft-v3", label: "Recraft V3", caps: caps(false, false, false, false), estimate_usd: 0.04 },
            ],
            text_models: vec![],
            note: "Any fal model id works. Kontext models take a reference or source image.",
        },
        ProviderInfo {
            id: "replicate",
            name: "Replicate",
            key_url: "https://replicate.com/account/api-tokens",
            key_hint: "r8_...",
            needs_key: true,
            needs_base_url: false,
            planning: false,
            image_models: vec![
                ModelInfo { id: "black-forest-labs/flux-1.1-pro", label: "FLUX1.1 [pro]", caps: caps(false, false, false, false), estimate_usd: 0.04 },
                ModelInfo { id: "black-forest-labs/flux-schnell", label: "FLUX.1 [schnell]", caps: caps(false, false, false, false), estimate_usd: 0.003 },
                ModelInfo { id: "black-forest-labs/flux-kontext-pro", label: "FLUX.1 Kontext [pro]", caps: caps(false, true, true, false), estimate_usd: 0.04 },
                ModelInfo { id: "ideogram-ai/ideogram-v3-turbo", label: "Ideogram 3 Turbo", caps: caps(false, false, false, false), estimate_usd: 0.03 },
                ModelInfo { id: "recraft-ai/recraft-v3", label: "Recraft V3", caps: caps(false, false, false, false), estimate_usd: 0.04 },
            ],
            text_models: vec![],
            note: "Any official model id (owner/name) works. Kontext models take a reference or source image.",
        },
        ProviderInfo {
            id: "stability",
            name: "Stability AI",
            key_url: "https://platform.stability.ai/account/keys",
            key_hint: "sk-...",
            needs_key: true,
            needs_base_url: false,
            planning: false,
            image_models: vec![
                ModelInfo { id: "core", label: "Stable Image Core", caps: caps(false, false, false, false), estimate_usd: 0.03 },
                ModelInfo { id: "ultra", label: "Stable Image Ultra", caps: caps(false, false, false, false), estimate_usd: 0.08 },
                ModelInfo { id: "sd3.5-large", label: "SD 3.5 Large", caps: caps(false, false, false, false), estimate_usd: 0.065 },
                ModelInfo { id: "sd3.5-medium", label: "SD 3.5 Medium", caps: caps(false, false, false, false), estimate_usd: 0.035 },
            ],
            text_models: vec![],
            note: "Plain text-to-image. Transparent sets use background keying.",
        },
        ProviderInfo {
            id: "compatible",
            name: "OpenAI-compatible",
            key_url: "",
            key_hint: "optional",
            needs_key: false,
            needs_base_url: true,
            planning: true,
            image_models: vec![],
            text_models: vec![],
            note: "Local servers and gateways that implement /images/generations (and optionally /images/edits and /responses).",
        },
    ]
}

/// Capabilities for a provider and model. Unknown models get conservative defaults.
pub fn caps_for(kind: Kind, model: &str) -> Caps {
    let known = catalog()
        .into_iter()
        .find(|p| p.id == kind.id())
        .and_then(|p| {
            p.image_models
                .into_iter()
                .find(|m| m.id == model)
                .map(|m| m.caps)
        });
    if let Some(c) = known {
        return c;
    }
    match kind {
        Kind::Openai => caps(true, true, true, true),
        Kind::Compatible => caps(true, false, false, true),
        Kind::Google => caps(
            false,
            model.starts_with("gemini"),
            model.starts_with("gemini"),
            false,
        ),
        Kind::Fal | Kind::Replicate => caps(
            false,
            model.contains("kontext"),
            model.contains("kontext"),
            false,
        ),
        Kind::Stability => Caps::default(),
    }
}

pub fn estimate_for(kind: Kind, model: &str) -> f64 {
    catalog()
        .into_iter()
        .find(|p| p.id == kind.id())
        .and_then(|p| {
            p.image_models
                .into_iter()
                .find(|m| m.id == model)
                .map(|m| m.estimate_usd)
        })
        .unwrap_or(0.04)
}

/// Everything a provider needs to draw one image.
#[derive(Debug, Clone)]
pub struct Draw {
    pub model: String,
    pub prompt: String,
    pub size: String,
    pub quality: String,
    /// "transparent", "opaque" or "auto", already adjusted for the model's capabilities.
    pub background: String,
    /// Style references or the source image of an edit, (file name, PNG bytes).
    pub images: Vec<(String, Vec<u8>)>,
}

pub struct Provider {
    pub kind: Kind,
    key: String,
    base: String,
    http: reqwest::Client,
}

impl Provider {
    pub fn new(kind: Kind, key: String, base_override: Option<String>) -> Provider {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .user_agent(concat!("eitri/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        let base = base_override.unwrap_or_else(|| default_base(kind).to_string());
        Provider {
            kind,
            key,
            base: base.trim_end_matches('/').to_string(),
            http,
        }
    }

    fn openai(&self) -> OpenAi {
        OpenAi::new(&self.base, self.key.clone())
    }

    pub async fn draw(&self, d: &Draw) -> Result<ImageResult, ApiError> {
        let r = match self.kind {
            Kind::Openai | Kind::Compatible => {
                let req = ImageRequest {
                    model: d.model.clone(),
                    prompt: d.prompt.clone(),
                    size: d.size.clone(),
                    quality: d.quality.clone(),
                    background: d.background.clone(),
                };
                if d.images.is_empty() {
                    self.openai().generate(&req).await
                } else {
                    self.openai().edit(&req, &d.images).await
                }
            }
            Kind::Google => google::draw(&self.http, &self.base, &self.key, d).await,
            Kind::Fal => fal::draw(&self.http, &self.base, &self.key, d).await,
            Kind::Replicate => replicate::draw(&self.http, &self.base, &self.key, d).await,
            Kind::Stability => stability::draw(&self.http, &self.base, &self.key, d).await,
        };
        r.map_err(|e| tag(self.kind, &d.model, e))
    }

    /// Streams a structured plan. Only providers with `planning: true` support this.
    pub async fn plan(
        &self,
        model: &str,
        system: &str,
        user: &str,
        schema: &Value,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(Value, Usage), ApiError> {
        match self.kind {
            Kind::Openai | Kind::Compatible => {
                self.openai()
                    .respond_json_stream(model, system, user, "image_spec", schema, on_text)
                    .await
            }
            Kind::Google => {
                google::plan(
                    &self.http, &self.base, &self.key, model, system, user, schema, on_text,
                )
                .await
            }
            _ => Err(ApiError::BadRequest(format!(
                "{} cannot write plans; pick OpenAI or Gemini for planning",
                self.kind.id()
            ))),
        }
        .map_err(|e| tag(self.kind, model, e))
    }

    /// Cheap call that proves the key works.
    pub async fn verify(&self) -> Result<(), ApiError> {
        let r = match self.kind {
            Kind::Openai | Kind::Compatible => self.openai().list_models().await.map(|_| ()),
            Kind::Google => google::verify(&self.http, &self.base, &self.key).await,
            Kind::Replicate => replicate::verify(&self.http, &self.base, &self.key).await,
            Kind::Stability => stability::verify(&self.http, &self.base, &self.key).await,
            // fal has no free authenticated endpoint; the key is checked on first use.
            Kind::Fal => Ok(()),
        };
        r.map_err(|e| tag(self.kind, "", e))
    }

    pub async fn list_models(&self) -> Result<Vec<String>, ApiError> {
        match self.kind {
            Kind::Openai | Kind::Compatible => self.openai().list_models().await,
            Kind::Google => google::list_models(&self.http, &self.base, &self.key).await,
            _ => Ok(vec![]),
        }
    }
}

pub fn default_base(kind: Kind) -> &'static str {
    match kind {
        Kind::Openai | Kind::Compatible => DEFAULT_BASE,
        Kind::Google => "https://generativelanguage.googleapis.com/v1beta",
        Kind::Fal => "https://fal.run",
        Kind::Replicate => "https://api.replicate.com/v1",
        Kind::Stability => "https://api.stability.ai/v2beta",
    }
}

/// Adds the provider and model to an error so a failure report says exactly what broke.
fn tag(kind: Kind, model: &str, e: ApiError) -> ApiError {
    let who = if model.is_empty() {
        kind.id().to_string()
    } else {
        format!("{} {model}", kind.id())
    };
    let wrap = |m: String| {
        if m.starts_with('[') {
            m
        } else {
            format!("[{who}] {m}")
        }
    };
    match e {
        ApiError::Auth(m) => ApiError::Auth(wrap(m)),
        ApiError::Quota(m) => ApiError::Quota(wrap(m)),
        ApiError::RateLimited(m) => ApiError::RateLimited(wrap(m)),
        ApiError::Refused(m) => ApiError::Refused(wrap(m)),
        ApiError::BadRequest(m) => ApiError::BadRequest(wrap(m)),
        ApiError::Transient(m) => ApiError::Transient(wrap(m)),
    }
}

// ---------- shared helpers for the non-OpenAI providers ----------

/// Classifies an HTTP failure from any provider by status and wording.
pub fn classify_generic(status: u16, body: &str) -> ApiError {
    let text = crate::error::redact(&format!(
        "{status}: {}",
        body.chars().take(400).collect::<String>()
    ));
    let lower = body.to_lowercase();
    let money = [
        "quota",
        "billing",
        "credit",
        "insufficient",
        "payment",
        "balance",
        "exhausted",
    ];
    if status == 402 || money.iter().any(|w| lower.contains(w)) && status != 400 {
        return ApiError::Quota(text);
    }
    let unsafe_words = [
        "nsfw",
        "safety",
        "content policy",
        "moderation",
        "blocked",
        "sensitive",
    ];
    match status {
        401 | 403 => ApiError::Auth(text),
        429 => ApiError::RateLimited(text),
        400 | 422 if unsafe_words.iter().any(|w| lower.contains(w)) => ApiError::Refused(text),
        400..=499 => ApiError::BadRequest(text),
        _ => ApiError::Transient(text),
    }
}

pub async fn read_error(res: reqwest::Response) -> ApiError {
    let status = res.status().as_u16();
    let body = res.text().await.unwrap_or_default();
    classify_generic(status, &body)
}

pub fn net(e: reqwest::Error) -> ApiError {
    ApiError::Transient(crate::error::redact(&e.to_string()))
}

/// Width and height from "1024x1536"-style sizes, defaulting to square.
pub fn dims(size: &str) -> (u32, u32) {
    let mut it = size
        .split('x')
        .map(|n| n.trim().parse::<u32>().unwrap_or(1024));
    (it.next().unwrap_or(1024), it.next().unwrap_or(1024))
}

pub fn aspect(size: &str) -> &'static str {
    let (w, h) = dims(size);
    if w > h {
        "3:2"
    } else if h > w {
        "2:3"
    } else {
        "1:1"
    }
}

pub fn data_uri(png: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

/// Downloads an image a provider returned by URL (or decodes a data URI) and normalises it to PNG.
pub async fn fetch_image(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = if let Some(b64) = url
        .strip_prefix("data:")
        .and_then(|r| r.split_once("base64,").map(|(_, d)| d))
    {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| ApiError::Transient(e.to_string()))?
    } else {
        let res = http.get(url).send().await.map_err(net)?;
        if !res.status().is_success() {
            return Err(read_error(res).await);
        }
        res.bytes().await.map_err(net)?.to_vec()
    };
    to_png(&bytes)
}

pub fn to_png(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    if bytes.starts_with(b"\x89PNG") {
        return Ok(bytes.to_vec());
    }
    let img = image::load_from_memory(bytes)
        .map_err(|e| ApiError::Transient(format!("could not read the returned image: {e}")))?;
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| ApiError::Transient(e.to_string()))?;
    Ok(out)
}

/// A usage record for providers that bill per image rather than per token.
pub fn per_image_usage() -> Usage {
    Usage {
        input_text: 0,
        input_image: 0,
        output: 0,
    }
}

pub fn json_or_empty(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_errors_classify() {
        assert!(matches!(
            classify_generic(401, "bad token"),
            ApiError::Auth(_)
        ));
        assert!(matches!(
            classify_generic(402, "pay up"),
            ApiError::Quota(_)
        ));
        assert!(matches!(
            classify_generic(403, "Insufficient credit balance"),
            ApiError::Quota(_)
        ));
        assert!(matches!(
            classify_generic(422, "NSFW content detected"),
            ApiError::Refused(_)
        ));
        assert!(matches!(
            classify_generic(429, "slow down"),
            ApiError::RateLimited(_)
        ));
        assert!(matches!(
            classify_generic(500, "oops"),
            ApiError::Transient(_)
        ));
    }

    #[test]
    fn sizes_map_to_aspects() {
        assert_eq!(aspect("1024x1024"), "1:1");
        assert_eq!(aspect("1536x1024"), "3:2");
        assert_eq!(aspect("1024x1536"), "2:3");
        assert_eq!(dims("junk"), (1024, 1024));
    }

    #[test]
    fn caps_known_and_unknown() {
        assert!(caps_for(Kind::Openai, "gpt-image-1").transparent);
        assert!(caps_for(Kind::Fal, "fal-ai/flux-pro/kontext").references);
        assert!(!caps_for(Kind::Replicate, "someone/some-model").references);
        assert!(caps_for(Kind::Google, "gemini-3-image-preview").references);
    }

    #[test]
    fn errors_name_the_provider() {
        let e = tag(
            Kind::Fal,
            "fal-ai/flux/dev",
            ApiError::BadRequest("nope".into()),
        );
        assert!(e.to_string().contains("[fal fal-ai/flux/dev] nope"));
    }
}
