use crate::securelog::entry::SecureLogEntry;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::env;

pub fn resolve_log_path() -> PathBuf {
    let cwd = env::current_dir().unwrap();
    cwd.join("logs").join("secure.log")
}

pub fn read_last_hash(path: &Path) -> String {
    if let Ok(content) = fs::read_to_string(path) {
        content.lines().rev().find_map(|line| {
            serde_json::from_str::<SecureLogEntry>(line)
                .ok()
                .map(|entry| entry.current_hash)
        }).unwrap_or_default()
    } else {
        String::new()
    }
}

pub fn read_secure_log_entries() -> Result<Vec<SecureLogEntry>, String> {
    let path = resolve_log_path();
    let content = fs::read_to_string(path).map_err(|e| format!("Error al leer secure.log: {}", e))?;
    Ok(content.lines()
        .filter_map(|line| serde_json::from_str::<SecureLogEntry>(line).ok())
        .collect())
}

pub fn read_secure_log_formatted() -> Vec<String> {
    match read_secure_log_entries() {
        Ok(entries) => entries.iter().map(|entry| {
            format!(
                "[{}] {}:\n{}",
                entry.timestamp,
                entry.event,
                serde_json::to_string_pretty(&entry.details).unwrap_or("{}".to_string())
            )
        }).collect(),
        Err(e) => vec![format!("Error leyendo secure.log: {}", e)],
    }
}
