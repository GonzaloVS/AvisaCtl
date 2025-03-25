use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct AvisaCtlConfig {
    pub last_server_address: String,
}

pub fn load_config() -> AvisaCtlConfig {
    confy::load("avisactl", None).unwrap_or_default()
}

pub fn save_config(cfg: &AvisaCtlConfig) {
    let _ = confy::store("avisactl", None, cfg);
}
