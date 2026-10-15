//! Stability AI Stable Image API (v2beta). Multipart form, raw image bytes back.
//! Model ids map to endpoints: `core`, `ultra`, and `sd3.5-*` through `/sd3`.

use super::{aspect, net, read_error, to_png, Draw};
use crate::openai::{ApiError, ImageResult};

pub fn endpoint(model: &str) -> (&'static str, Option<&str>) {
    match model {
        "ultra" => ("stable-image/generate/ultra", None),
        m if m.starts_with("sd3") => ("stable-image/generate/sd3", Some(m)),
        _ => ("stable-image/generate/core", None),
    }
}

pub async fn draw(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    d: &Draw,
) -> Result<ImageResult, ApiError> {
    let (path, sd3_model) = endpoint(&d.model);
    let mut form = reqwest::multipart::Form::new()
        .text("prompt", d.prompt.clone())
        .text("output_format", "png")
        .text("aspect_ratio", aspect(&d.size).to_string());
    if let Some(m) = sd3_model {
        form = form.text("model", m.to_string());
    }
    let res = http
        .post(format!("{base}/{path}"))
        .bearer_auth(key)
        .header("Accept", "image/*")
        .multipart(form)
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    if res
        .headers()
        .get("finish-reason")
        .and_then(|v| v.to_str().ok())
        == Some("CONTENT_FILTERED")
    {
        return Err(ApiError::Refused(
            "Stability's content filter blocked the image".into(),
        ));
    }
    let bytes = res.bytes().await.map_err(net)?;
    Ok(ImageResult {
        png: to_png(&bytes)?,
        usage: super::per_image_usage(),
    })
}

pub async fn verify(http: &reqwest::Client, base: &str, key: &str) -> Result<(), ApiError> {
    // Account endpoints live under v1.
    let root = base.trim_end_matches("/v2beta");
    let res = http
        .get(format!("{root}/v1/user/balance"))
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
    use super::endpoint;

    #[test]
    fn model_ids_pick_endpoints() {
        assert_eq!(endpoint("core").0, "stable-image/generate/core");
        assert_eq!(endpoint("ultra").0, "stable-image/generate/ultra");
        assert_eq!(
            endpoint("sd3.5-large"),
            ("stable-image/generate/sd3", Some("sd3.5-large"))
        );
    }
}
