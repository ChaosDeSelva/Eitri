//! Thin OpenAI client: image generation, image edits with reference images, structured
//! JSON responses for planning, and the model list. Every failure is classified so the
//! runner knows whether to retry, skip the item, or stop the whole run.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE: &str = "https://api.openai.com/v1";

#[derive(Debug, thiserror::Error, Clone, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum ApiError {
    /// 401/403: bad, expired or unverified key. Stops every run.
    #[error("API key rejected: {0}")]
    Auth(String),
    /// Account out of credit or at its billing limit. Stops every run.
    #[error("Account quota or billing limit reached: {0}")]
    Quota(String),
    /// 429 rate limit. Retry after a pause.
    #[error("Rate limited: {0}")]
    RateLimited(String),
    /// The prompt or image was refused by the safety system. Needs a person.
    #[error("Refused by the content policy: {0}")]
    Refused(String),
    /// Malformed request (bad model, size, parameter). Retrying will not help.
    #[error("Request rejected: {0}")]
    BadRequest(String),
    /// Network trouble or a 5xx. Retry.
    #[error("Request failed: {0}")]
    Transient(String),
}

impl ApiError {
    pub fn stops_run(&self) -> bool {
        matches!(self, ApiError::Auth(_) | ApiError::Quota(_))
    }
    pub fn retryable(&self) -> bool {
        matches!(self, ApiError::RateLimited(_) | ApiError::Transient(_))
    }
}

/// Token usage split the way image models bill it.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_text: u64,
    pub input_image: u64,
    pub output: u64,
}

impl Usage {
    fn from_json(v: &Value) -> Usage {
        let u = &v["usage"];
        let input = u["input_tokens"].as_u64().unwrap_or(0);
        let image = u["input_tokens_details"]["image_tokens"]
            .as_u64()
            .unwrap_or(0);
        let text = u["input_tokens_details"]["text_tokens"]
            .as_u64()
            .unwrap_or(input.saturating_sub(image));
        Usage {
            input_text: text,
            input_image: image,
            output: u["output_tokens"].as_u64().unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageRequest {
    pub model: String,
    pub prompt: String,
    pub size: String,
    pub quality: String,
    pub background: String,
}

#[derive(Debug, Clone)]
pub struct ImageResult {
    pub png: Vec<u8>,
    pub usage: Usage,
}

#[derive(Clone)]
pub struct OpenAi {
    http: reqwest::Client,
    base: String,
    key: String,
}

impl OpenAi {
    pub fn new(base: &str, key: String) -> OpenAi {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .user_agent(concat!("eitri/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        OpenAi {
            http,
            base: base.trim_end_matches('/').to_string(),
            key,
        }
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Result<Value, ApiError> {
        let res = rb
            .bearer_auth(&self.key)
            .send()
            .await
            .map_err(|e| ApiError::Transient(crate::error::redact(&e.to_string())))?;
        let status = res.status().as_u16();
        let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
        if (200..300).contains(&status) {
            return Ok(body);
        }
        Err(classify(status, &body))
    }

    pub async fn list_models(&self) -> Result<Vec<String>, ApiError> {
        let v = self
            .send(self.http.get(format!("{}/models", self.base)))
            .await?;
        let mut ids: Vec<String> = v["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        Ok(ids)
    }

    pub async fn generate(&self, req: &ImageRequest) -> Result<ImageResult, ApiError> {
        let body = json!({
            "model": req.model, "prompt": req.prompt, "n": 1, "size": req.size,
            "quality": req.quality, "background": req.background, "output_format": "png",
        });
        let v = self
            .send(
                self.http
                    .post(format!("{}/images/generations", self.base))
                    .json(&body),
            )
            .await?;
        decode_image(&v)
    }

    /// Image edit: `images` are reference or source PNGs (name, bytes).
    pub async fn edit(
        &self,
        req: &ImageRequest,
        images: &[(String, Vec<u8>)],
    ) -> Result<ImageResult, ApiError> {
        let mut form = reqwest::multipart::Form::new()
            .text("model", req.model.clone())
            .text("prompt", req.prompt.clone())
            .text("size", req.size.clone())
            .text("quality", req.quality.clone())
            .text("background", req.background.clone())
            .text("output_format", "png");
        for (name, bytes) in images {
            let part = reqwest::multipart::Part::bytes(bytes.clone())
                .file_name(name.clone())
                .mime_str("image/png")
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
            form = form.part("image[]", part);
        }
        let v = self
            .send(
                self.http
                    .post(format!("{}/images/edits", self.base))
                    .multipart(form),
            )
            .await?;
        decode_image(&v)
    }

    /// One structured-output call through the Responses API.
    pub async fn respond_json(
        &self,
        model: &str,
        system: &str,
        user: &str,
        schema_name: &str,
        schema: &Value,
    ) -> Result<(Value, Usage), ApiError> {
        let body = json!({
            "model": model,
            "input": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "text": {"format": {"type": "json_schema", "name": schema_name, "schema": schema, "strict": true}}
        });
        let v = self
            .send(
                self.http
                    .post(format!("{}/responses", self.base))
                    .json(&body),
            )
            .await?;
        let text = v["output"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|o| o["content"].as_array().into_iter().flatten())
            .find_map(|c| {
                (c["type"] == "output_text").then(|| c["text"].as_str().unwrap_or("").to_string())
            })
            .ok_or_else(|| ApiError::Transient("the planner returned no text".into()))?;
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| ApiError::Transient(format!("the planner returned invalid JSON: {e}")))?;
        Ok((parsed, Usage::from_json(&v)))
    }
}

impl OpenAi {
    /// Same as `respond_json`, but streams: `on_text` receives the full text so far after
    /// every delta, so the UI can show the plan being written instead of a frozen spinner.
    pub async fn respond_json_stream(
        &self,
        model: &str,
        system: &str,
        user: &str,
        schema_name: &str,
        schema: &Value,
        on_text: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(Value, Usage), ApiError> {
        let body = json!({
            "model": model,
            "stream": true,
            "input": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ],
            "text": {"format": {"type": "json_schema", "name": schema_name, "schema": schema, "strict": true}}
        });
        let mut res = self
            .http
            .post(format!("{}/responses", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Transient(crate::error::redact(&e.to_string())))?;
        let status = res.status().as_u16();
        if !(200..300).contains(&status) {
            let body: Value = res.json().await.unwrap_or_else(|_| json!({}));
            return Err(classify(status, &body));
        }
        let mut sse = SseText::default();
        let mut buf = String::new();
        while let Some(chunk) = res
            .chunk()
            .await
            .map_err(|e| ApiError::Transient(crate::error::redact(&e.to_string())))?
        {
            buf.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buf.find("\n\n") {
                let event: String = buf.drain(..end + 2).collect();
                if sse.feed(&event)? {
                    on_text(&sse.text);
                }
            }
        }
        if !buf.trim().is_empty() && sse.feed(&buf)? {
            on_text(&sse.text);
        }
        let parsed: Value = serde_json::from_str(&sse.text)
            .map_err(|e| ApiError::Transient(format!("the planner returned invalid JSON: {e}")))?;
        Ok((parsed, sse.usage))
    }
}

/// Accumulates a Responses API event stream.
#[derive(Default)]
pub struct SseText {
    pub text: String,
    pub usage: Usage,
}

impl SseText {
    /// Feeds one SSE event block. Returns true when the text grew.
    pub fn feed(&mut self, block: &str) -> Result<bool, ApiError> {
        let data: String = block
            .lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .map(|l| l.trim_start())
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            return Ok(false);
        }
        let Ok(v) = serde_json::from_str::<Value>(&data) else {
            return Ok(false);
        };
        match v["type"].as_str().unwrap_or("") {
            "response.output_text.delta" => {
                self.text.push_str(v["delta"].as_str().unwrap_or(""));
                Ok(true)
            }
            "response.completed" => {
                self.usage = Usage::from_json(&v["response"]);
                Ok(false)
            }
            "response.failed" | "response.incomplete" | "error" => {
                let err = if v["error"].is_object() {
                    v["error"].clone()
                } else {
                    v["response"]["error"].clone()
                };
                let reason = v["response"]["incomplete_details"]["reason"]
                    .as_str()
                    .unwrap_or("");
                if err.is_null() && !reason.is_empty() {
                    return Err(ApiError::Transient(format!(
                        "the plan was cut short ({reason}); try fewer images"
                    )));
                }
                Err(classify(400, &json!({ "error": err })))
            }
            _ => Ok(false),
        }
    }
}

fn decode_image(v: &Value) -> Result<ImageResult, ApiError> {
    let b64 = v["data"][0]["b64_json"]
        .as_str()
        .ok_or_else(|| ApiError::Transient("response had no image".into()))?;
    let png = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| ApiError::Transient(format!("image was not valid base64: {e}")))?;
    Ok(ImageResult {
        png,
        usage: Usage::from_json(v),
    })
}

pub fn classify(status: u16, body: &Value) -> ApiError {
    let err = &body["error"];
    let code = err["code"]
        .as_str()
        .or_else(|| err["type"].as_str())
        .unwrap_or("");
    let message = crate::error::redact(err["message"].as_str().unwrap_or("no details"));
    let text = format!("{status} {code}: {message}");
    let lower = message.to_lowercase();
    if code == "insufficient_quota"
        || code == "billing_hard_limit_reached"
        || lower.contains("billing")
    {
        return ApiError::Quota(text);
    }
    match status {
        401 | 403 => ApiError::Auth(text),
        429 => ApiError::RateLimited(text),
        400 if code == "moderation_blocked"
            || code == "content_policy_violation"
            || lower.contains("safety system") =>
        {
            ApiError::Refused(text)
        }
        400..=499 => ApiError::BadRequest(text),
        _ => ApiError::Transient(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_stop_conditions() {
        let expired = json!({"error": {"code": "expired_secret_key", "message": "Your API key has expired."}});
        assert!(matches!(classify(401, &expired), ApiError::Auth(_)));
        let quota = json!({"error": {"code": "insufficient_quota", "message": "You exceeded your current quota"}});
        let q = classify(429, &quota);
        assert!(matches!(q, ApiError::Quota(_)) && q.stops_run());
        let rate = json!({"error": {"code": "rate_limit_exceeded", "message": "slow down"}});
        assert!(classify(429, &rate).retryable());
        let refused = json!({"error": {"code": "moderation_blocked", "message": "rejected by the safety system"}});
        assert!(matches!(classify(400, &refused), ApiError::Refused(_)));
        assert!(matches!(classify(400, &json!({})), ApiError::BadRequest(_)));
        assert!(classify(502, &json!({})).retryable());
    }

    #[test]
    fn sse_accumulates_deltas_and_usage() {
        let mut s = SseText::default();
        assert!(s.feed("event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"a\\\":\"}\n\n").unwrap());
        assert!(s
            .feed("data: {\"type\":\"response.output_text.delta\",\"delta\":\"1}\"}")
            .unwrap());
        assert!(!s.feed("data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}}").unwrap());
        assert_eq!(s.text, "{\"a\":1}");
        assert_eq!(s.usage.output, 5);
        let failed = s.feed("data: {\"type\":\"error\",\"error\":{\"code\":\"insufficient_quota\",\"message\":\"no\"}}");
        assert!(matches!(failed, Err(ApiError::Quota(_))));
    }

    #[test]
    fn usage_splits_image_and_text_tokens() {
        let v = json!({"usage": {"input_tokens": 900, "output_tokens": 1393,
            "input_tokens_details": {"image_tokens": 400, "text_tokens": 500}}});
        assert_eq!(
            Usage::from_json(&v),
            Usage {
                input_text: 500,
                input_image: 400,
                output: 1393
            }
        );
    }
}
