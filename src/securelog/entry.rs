use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureLogEntry {
    pub timestamp: String,
    pub event: String,
    pub details: Value,
    pub prev_hash: String,
    pub current_hash: String,
    pub gpg_signature: Option<String>,
    pub tsa_timestamp: Option<String>,
}
