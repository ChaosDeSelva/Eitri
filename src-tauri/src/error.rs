use crate::openai::ApiError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("keychain: {0}")]
    Keychain(String),
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("image: {0}")]
    Image(#[from] image::ImageError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type AppResult<T> = Result<T, AppError>;

pub fn msg(s: impl Into<String>) -> AppError {
    AppError::Msg(s.into())
}

// Errors cross into the UI and the log as plain strings, always redacted.
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&redact(&self.to_string()))
    }
}

/// Masks anything that looks like an API key (`sk-...`) or a bearer token.
pub fn redact(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &input[i..];
        let marker = if rest.starts_with("sk-") {
            Some(3)
        } else if rest.starts_with("Bearer ") {
            Some(7)
        } else {
            None
        };
        if let Some(skip) = marker {
            let token_len = rest[skip..]
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len() - skip);
            if token_len >= 8 {
                out.push_str(&rest[..skip]);
                out.push_str("[redacted]");
                i += skip + token_len;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn masks_keys_and_bearer_tokens() {
        let s = "bad key sk-proj-AbC123_def-456xyz and Bearer abcdefghijkl done";
        let r = redact(s);
        assert!(!r.contains("AbC123"));
        assert!(!r.contains("abcdefghijkl"));
        assert!(r.contains("sk-[redacted]"));
        assert!(r.contains("Bearer [redacted]"));
        assert!(r.ends_with("done"));
    }

    #[test]
    fn leaves_short_lookalikes() {
        assert_eq!(redact("task sk-1 ok"), "task sk-1 ok");
    }
}
