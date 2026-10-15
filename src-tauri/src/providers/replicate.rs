//! Replicate official-model predictions:
//! `POST /v1/models/{owner}/{name}/predictions` with `Prefer: wait`, then poll `urls.get`
//! if the prediction has not finished inside the wait window.

use super::{aspect, data_uri, fetch_image, net, read_error, Draw};
use crate::openai::{ApiError, ImageResult};
use serde_json::{json, Value};
use std::time::Duration;

pub fn input(d: &Draw) -> Value {
    let mut i = json!({
        "prompt": d.prompt,
        "aspect_ratio": aspect(&d.size),
        "output_format": "png",
    });
    if let Some((_, bytes)) = d.images.first() {
        // Kontext models call it input_image; harmless for models that ignore it.
        i["input_image"] = json!(data_uri(bytes));
    }
    i
}

fn output_url(v: &Value) -> Option<String> {
    match &v["output"] {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => a.first().and_then(|x| x.as_str()).map(String::from),
        _ => None,
    }
}

pub async fn draw(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    d: &Draw,
) -> Result<ImageResult, ApiError> {
    let res = http
        .post(format!(
            "{base}/models/{}/predictions",
            d.model.trim_matches('/')
        ))
        .bearer_auth(key)
        .header("Prefer", "wait=60")
        .json(&json!({ "input": input(d) }))
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    let mut v: Value = res.json().await.map_err(net)?;
    for _ in 0..90 {
        match v["status"].as_str().unwrap_or("") {
            "succeeded" => break,
            "failed" | "canceled" => {
                let err = v["error"]
                    .as_str()
                    .unwrap_or("prediction failed")
                    .to_string();
                let lower = err.to_lowercase();
                return Err(
                    if lower.contains("nsfw")
                        || lower.contains("safety")
                        || lower.contains("sensitive")
                    {
                        ApiError::Refused(err)
                    } else {
                        ApiError::BadRequest(err)
                    },
                );
            }
            _ => {}
        }
        if output_url(&v).is_some() && v["status"] == "succeeded" {
            break;
        }
        let Some(get) = v["urls"]["get"].as_str().map(String::from) else {
            break;
        };
        tokio::time::sleep(Duration::from_secs(2)).await;
        let r = http.get(&get).bearer_auth(key).send().await.map_err(net)?;
        if !r.status().is_success() {
            return Err(read_error(r).await);
        }
        v = r.json().await.map_err(net)?;
    }
    let url = output_url(&v)
        .ok_or_else(|| ApiError::Transient(format!("no output (status {})", v["status"])))?;
    Ok(ImageResult {
        png: fetch_image(http, &url).await?,
        usage: super::per_image_usage(),
    })
}

pub async fn verify(http: &reqwest::Client, base: &str, key: &str) -> Result<(), ApiError> {
    let res = http
        .get(format!("{base}/account"))
        .bearer_auth(key)
        .send()
        .await
        .map_err(net)?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(read_error(res).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_shapes() {
        assert_eq!(
            output_url(&json!({"output": "https://x/a.png"})).as_deref(),
            Some("https://x/a.png")
        );
        assert_eq!(
            output_url(&json!({"output": ["https://x/b.png"]})).as_deref(),
            Some("https://x/b.png")
        );
        assert_eq!(output_url(&json!({"output": null})), None);
    }
}
