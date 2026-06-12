//! App-lock (PIN) commands.
//!
//! The PIN itself is never stored — only a salted SHA-256 verifier, kept in the
//! OS keychain. The keychain is the real security boundary; the salted hash adds
//! defence in depth.

use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use tauri::State;
use uuid::Uuid;

use harbor_core::application::ports::SecretRef;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

fn hash_pin(salt: &str, pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(pin.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tauri::command]
pub async fn has_app_pin(state: State<'_, AppState>) -> CommandResult<bool> {
    Ok(state.secrets.get(&SecretRef::AppPin).await?.is_some())
}

#[tauri::command]
pub async fn set_app_pin(state: State<'_, AppState>, pin: String) -> CommandResult<()> {
    if pin.len() < 4 || pin.len() > 12 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(CommandError::new(
            "validation",
            "PIN must be 4–12 digits",
        ));
    }
    let salt = Uuid::new_v4().to_string();
    let stored = format!("{salt}${}", hash_pin(&salt, &pin));
    state
        .secrets
        .set(&SecretRef::AppPin, SecretString::from(stored))
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn verify_app_pin(state: State<'_, AppState>, pin: String) -> CommandResult<bool> {
    match state.secrets.get(&SecretRef::AppPin).await? {
        // No PIN configured → nothing to unlock.
        None => Ok(true),
        Some(stored) => Ok(match stored.expose_secret().split_once('$') {
            Some((salt, hash)) => hash_pin(salt, &pin) == hash,
            None => false,
        }),
    }
}

/// Remove the PIN lock. Requires the current PIN as a safeguard.
#[tauri::command]
pub async fn clear_app_pin(state: State<'_, AppState>, current_pin: String) -> CommandResult<()> {
    if let Some(stored) = state.secrets.get(&SecretRef::AppPin).await? {
        if let Some((salt, hash)) = stored.expose_secret().split_once('$') {
            if hash_pin(salt, &current_pin) != hash {
                return Err(CommandError::new("validation", "incorrect PIN"));
            }
        }
    }
    state.secrets.delete(&SecretRef::AppPin).await?;
    Ok(())
}
