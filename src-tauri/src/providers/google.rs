//! Google Gemini API (AI Studio keys).
//! - Gemini image models: `models/{model}:generateContent` with inline reference images.
//! - Imagen: `models/{model}:predict`.
//! - Planning: `models/{model}:streamGenerateContent?alt=sse` with a JSON response schema.

use super::{aspect, net, read_error, to_png, Draw};
use crate::openai::{ApiError, ImageResult, Usage};
use base64::Engine;
use serde_json::{json, Value};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn usage(v: &Value) -> Usage {
    let u = &v["usageMetadata"];
    Usage {
        input_text: u["promptTokenCount"].as_u64().unwrap_or(0),
        input_image: 0,
        output: u["candidatesTokenCount"].as_u64().unwrap_or(0),
    }
}

pub async fn draw(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    d: &Draw,
) -> Result<ImageResult, ApiError> {
    if d.model.starts_with("imagen") {
        return imagen(http, base, key, d).await;
    }
    let mut parts = vec![json!({ "text": d.prompt })];
    for (_, bytes) in &d.images {
        parts.push(json!({ "inline_data": { "mime_type": "image/png", "data": b64(bytes) } }));
    }
    let body = json!({
        "contents": [{ "role": "user", "parts": parts }],
        "generationConfig": {
            "responseModalities": ["IMAGE", "TEXT"],
            "imageConfig": { "aspectRatio": aspect(&d.size) }
        }
    });
    let res = http
        .post(format!("{base}/models/{}:generateContent", d.model))
        .header("x-goog-api-key", key)
        .json(&body)
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    let v: Value = res.json().await.map_err(net)?;
    if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
        return Err(ApiError::Refused(format!("blocked: {reason}")));
    }
    let parts = v["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let data = parts
        .iter()
        .find_map(|p| {
            p["inlineData"]["data"]
                .as_str()
                .or_else(|| p["inline_data"]["data"].as_str())
        })
        .ok_or_else(|| {
            let finish = v["candidates"][0]["finishReason"].as_str().unwrap_or("");
            if matches!(finish, "SAFETY" | "PROHIBITED_CONTENT" | "IMAGE_SAFETY") {
                ApiError::Refused(format!("blocked: {finish}"))
            } else {
                ApiError::Transient(format!("no image in the response ({finish})"))
            }
        })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| ApiError::Transient(e.to_string()))?;
    Ok(ImageResult {
        png: to_png(&bytes)?,
        usage: usage(&v),
    })
}

async fn imagen(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    d: &Draw,
) -> Result<ImageResult, ApiError> {
    let body = json!({
        "instances": [{ "prompt": d.prompt }],
        "parameters": { "sampleCount": 1, "aspectRatio": aspect(&d.size) }
    });
    let res = http
        .post(format!("{base}/models/{}:predict", d.model))
        .header("x-goog-api-key", key)
        .json(&body)
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    let v: Value = res.json().await.map_err(net)?;
    let data = v["predictions"][0]["bytesBase64Encoded"]
        .as_str()
        .ok_or_else(|| {
            ApiError::Refused("Imagen returned no image (often a safety filter)".into())
        })?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| ApiError::Transient(e.to_string()))?;
    Ok(ImageResult {
        png: to_png(&bytes)?,
        usage: super::per_image_usage(),
    })
}

/// Gemini's response schema is an OpenAPI subset: it rejects `additionalProperties`.
pub fn gemini_schema(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(k, _)| k.as_str() != "additionalProperties")
                .map(|(k, v)| (k.clone(), gemini_schema(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(gemini_schema).collect()),
        other => other.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn plan(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    model: &str,
    system: &str,
    user: &str,
    schema: &Value,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<(Value, Usage), ApiError> {
    let body = json!({
        "systemInstruction": { "parts": [{ "text": system }] },
        "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        "generationConfig": { "responseMimeType": "application/json", "responseSchema": gemini_schema(schema) }
    });
    let mut res = http
        .post(format!(
            "{base}/models/{model}:streamGenerateContent?alt=sse"
        ))
        .header("x-goog-api-key", key)
        .json(&body)
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    let mut text = String::new();
    let mut last = Usage::default();
    let mut buf = String::new();
    while let Some(chunk) = res.chunk().await.map_err(net)? {
        buf.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(end) = buf.find("\n\n") {
            let block: String = buf.drain(..end + 2).collect();
            if feed(&block, &mut text, &mut last) {
                on_text(&text);
            }
        }
    }
    if feed(&buf, &mut text, &mut last) {
        on_text(&text);
    }
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::Transient(format!("the planner returned invalid JSON: {e}")))?;
    Ok((parsed, last))
}

fn feed(block: &str, text: &mut String, last: &mut Usage) -> bool {
    let data: String = block
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(str::trim_start)
        .collect();
    let Ok(v) = serde_json::from_str::<Value>(&data) else {
        return false;
    };
    if v["usageMetadata"].is_object() {
        *last = usage(&v);
    }
    let delta: String = v["candidates"][0]["content"]["parts"]
        .as_array()
        .map(|ps| ps.iter().filter_map(|p| p["text"].as_str()).collect())
        .unwrap_or_default();
    text.push_str(&delta);
    !delta.is_empty()
}

pub async fn verify(http: &reqwest::Client, base: &str, key: &str) -> Result<(), ApiError> {
    list_models(http, base, key).await.map(|_| ())
}

pub async fn list_models(
    http: &reqwest::Client,
    base: &str,
    key: &str,
) -> Result<Vec<String>, ApiError> {
    let res = http
        .get(format!("{base}/models?pageSize=200"))
        .header("x-goog-api-key", key)
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    let v: Value = res.json().await.map_err(net)?;
    Ok(v["models"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    m["name"]
                        .as_str()
                        .map(|n| n.trim_start_matches("models/").to_string())
                })
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_drops_additional_properties_everywhere() {
        let s = gemini_schema(&crate::planner::schema());
        assert!(!s.to_string().contains("additionalProperties"));
        assert_eq!(s["properties"]["items"]["items"]["required"][0], "slug");
    }

    #[test]
    fn stream_blocks_accumulate() {
        let mut t = String::new();
        let mut u = Usage::default();
        assert!(feed(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"{\\\"a\\\"\"}]}}]}",
            &mut t,
            &mut u
        ));
        assert!(feed("data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\":1}\"}]}}],\"usageMetadata\":{\"promptTokenCount\":3,\"candidatesTokenCount\":4}}", &mut t, &mut u));
        assert_eq!(t, "{\"a\":1}");
        assert_eq!(u.output, 4);
    }
}
