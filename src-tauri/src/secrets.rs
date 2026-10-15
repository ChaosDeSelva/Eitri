//! API keys live only in the OS credential store (macOS Keychain, Windows Credential Manager,
//! Secret Service on Linux), one entry per provider. They are never written to the database or
//! the log, or sent to the UI: the UI only learns whether a key exists and its last four characters.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::collections::BTreeMap;

const SERVICE: &str = "dev.eitri.app";

#[derive(Serialize, Clone, Debug)]
pub struct KeyStatus {
    pub has_key: bool,
    pub last4: Option<String>,
}

fn entry(provider: &str) -> AppResult<keyring::Entry> {
    let provider = crate::providers::Kind::parse(provider).id();
    keyring::Entry::new(SERVICE, &format!("{provider}-api-key"))
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn get_key(provider: &str) -> AppResult<Option<String>> {
    match entry(provider)?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn set_key(provider: &str, key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.len() < 8 {
        return Err(crate::error::msg("That does not look like an API key."));
    }
    entry(provider)?
        .set_password(key)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn delete_key(provider: &str) -> AppResult<()> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn status(provider: &str) -> AppResult<KeyStatus> {
    Ok(match get_key(provider)? {
        Some(k) => KeyStatus {
            has_key: true,
            last4: Some(
                k.chars()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect(),
            ),
        },
        None => KeyStatus {
            has_key: false,
            last4: None,
        },
    })
}

pub fn status_all() -> AppResult<BTreeMap<String, KeyStatus>> {
    crate::providers::Kind::ALL
        .iter()
        .map(|k| Ok((k.id().to_string(), status(k.id())?)))
        .collect()
}
