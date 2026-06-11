//! Error type returned from Tauri commands to the frontend.

use serde::Serialize;

use harbor_core::HarborError;

/// A serialisable command error. The frontend receives `{ code, message }` and
/// can branch on the stable `code` (e.g. to show a passphrase prompt) without
/// parsing human-readable text.
#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<HarborError> for CommandError {
    fn from(e: HarborError) -> Self {
        CommandError {
            code: e.code().to_string(),
            message: e.to_string(),
        }
    }
}

impl From<uuid::Error> for CommandError {
    fn from(e: uuid::Error) -> Self {
        CommandError::new("validation", format!("invalid id: {e}"))
    }
}

/// Convenience alias for command results.
pub type CommandResult<T> = std::result::Result<T, CommandError>;
