use std::fs;
use std::path::Path;

#[derive(Debug, PartialEq, Clone)]
pub enum Platform {
    Linux,
    Windows,
}

#[derive(Debug, Clone)]
pub struct RemoteConfig {
    pub server_address: String,
    pub username: String,
    pub pass: String,
    pub remote_path: String,
    pub secure_log_path: Option<String>,
}

pub fn extract_package_name(cargo_toml_path: &Path) -> Option<String> {
    let contents = fs::read_to_string(cargo_toml_path).ok()?;
    let mut inside_package = false;

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("[package]") {
            inside_package = true;
        } else if inside_package && line.starts_with("name") {
            return line
                .split('=')
                .nth(1)
                .map(|s| s.trim().trim_matches('"').to_string());
        }
    }

    None
}
