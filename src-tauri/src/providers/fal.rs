//! fal.ai synchronous endpoint: `POST https://fal.run/{model}` with `Authorization: Key ...`.
//! Kontext models take the reference or source image as `image_url` (a data URI works).

use super::{data_uri, dims, fetch_image, net, read_error, Draw};
use crate::openai::{ApiError, ImageResult};
use serde_json::{json, Value};

pub fn body(d: &Draw) -> Value {
    let (w, h) = dims(&d.size);
    let mut b = json!({
        "prompt": d.prompt,
        "num_images": 1,
        "output_format": "png",
        "enable_safety_checker": true,
    });
    if d.model.contains("kontext") {
        b["aspect_ratio"] = json!(super::aspect(&d.size));
        if let Some((_, bytes)) = d.images.first() {
            b["image_url"] = json!(data_uri(bytes));
        }
    } else {
        b["image_size"] = if w == h {
            json!("square_hd")
        } else {
            json!({ "width": w, "height": h })
        };
    }
    b
}

pub async fn draw(
    http: &reqwest::Client,
    base: &str,
    key: &str,
    d: &Draw,
) -> Result<ImageResult, ApiError> {
    let res = http
        .post(format!("{base}/{}", d.model.trim_start_matches('/')))
        .header("Authorization", format!("Key {key}"))
        .json(&body(d))
        .send()
        .await
        .map_err(net)?;
    if !res.status().is_success() {
        return Err(read_error(res).await);
    }
    let v: Value = res.json().await.map_err(net)?;
    if v["has_nsfw_concepts"]
        .as_array()
        .is_some_and(|a| a.iter().any(|x| x.as_bool() == Some(true)))
    {
        return Err(ApiError::Refused(
            "fal's safety checker blocked the image".into(),
        ));
    }
    let url = v["images"][0]["url"]
        .as_str()
        .or_else(|| v["image"]["url"].as_str())
        .ok_or_else(|| ApiError::Transient("fal returned no image".into()))?;
    Ok(ImageResult {
        png: fetch_image(http, url).await?,
        usage: super::per_image_usage(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(model: &str, size: &str, images: Vec<(String, Vec<u8>)>) -> Draw {
        Draw {
            model: model.into(),
            prompt: "p".into(),
            size: size.into(),
            quality: "medium".into(),
            background: "opaque".into(),
            images,
        }
    }

    #[test]
    fn square_and_landscape_sizes() {
        assert_eq!(
            body(&draw("fal-ai/flux/dev", "1024x1024", vec![]))["image_size"],
            "square_hd"
        );
        assert_eq!(
            body(&draw("fal-ai/flux/dev", "1536x1024", vec![]))["image_size"]["width"],
            1536
        );
    }

    #[test]
    fn kontext_sends_the_reference() {
        let b = body(&draw(
            "fal-ai/flux-pro/kontext",
            "1024x1024",
            vec![("r.png".into(), vec![1, 2, 3])],
        ));
        assert!(b["image_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(b.get("image_size").is_none());
    }
}
