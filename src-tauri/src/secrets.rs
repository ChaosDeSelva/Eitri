//! The API key lives only in the OS credential store (macOS Keychain, Windows Credential
//! Manager, Secret Service on Linux). It is never written to the database, the log, or
//! sent to the UI: the UI only learns whether a key exists and its last four characters.

use crate::error::{AppError, AppResult};
use serde::Serialize;

const SERVICE: &str = "dev.eitri.app";
const ACCOUNT: &str = "openai-api-key";

#[derive(Serialize, Clone, Debug)]
pub struct KeyStatus {
    pub has_key: bool,
    pub last4: Option<String>,
}

fn entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn get_key() -> AppResult<Option<String>> {
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn set_key(key: &str) -> AppResult<()> {
    let key = key.trim();
    if key.len() < 20 {
        return Err(crate::error::msg("That does not look like an API key."));
    }
    entry()?
        .set_password(key)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn delete_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn status() -> AppResult<KeyStatus> {
    Ok(match get_key()? {
        Some(k) => KeyStatus {
            has_key: true,
            last4: Some(k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect()),
        },
        None => KeyStatus { has_key: false, last4: None },
    })
}
