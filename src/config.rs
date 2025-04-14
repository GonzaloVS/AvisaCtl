use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AvisaCtlConfig {
    pub last_local_path: String,
    pub last_server_address: String,
    pub last_remote_user: String,
    pub last_remote_pass: String,
    pub last_remote_path: String,
    pub secure_log_path: Option<String>,
}

pub fn load_config() -> AvisaCtlConfig {
    confy::load("avisactl", None).unwrap_or_default()
}

pub fn save_config(config: &AvisaCtlConfig) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write("config.json", serialized).map_err(|e| e.to_string())?;
    Ok(())
}
