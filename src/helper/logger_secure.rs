use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

const LOG_DIR: &str = "/var/log/avisactl";
const LOG_FILE: &str = "secure.log";

pub struct SecureLogger {
    last_hash: Arc<Mutex<String>>,
}

impl SecureLogger {
    pub fn new() -> Self {
        fs::create_dir_all(LOG_DIR).unwrap();
        Self {
            last_hash: Arc::new(Mutex::new(read_last_hash())),
        }
    }

    pub fn log(&self, event: &str, details: serde_json::Value) {
        let now = Utc::now().to_rfc3339();
        let prev_hash = self.last_hash.lock().unwrap().clone();

        let content = json!({
            "timestamp": now,
            "event": event,
            "details": details,
            "prev_hash": prev_hash
        });

        let serialized = content.to_string();

        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let current_hash = format!("{:x}", hasher.finalize());

        *self.last_hash.lock().unwrap() = current_hash.clone();

        let full = json!({
            "timestamp": now,
            "event": event,
            "details": details,
            "prev_hash": prev_hash,
            "current_hash": current_hash
        });

        let filepath = format!("{}/{}", LOG_DIR, LOG_FILE);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filepath)
            .unwrap();

        writeln!(file, "{}", full.to_string()).unwrap();
    }
}

pub fn log_both(
    logs: &mut Vec<String>,
    secure_logger: &SecureLogger,
    msg: impl Into<String>,
    tag: &str,
) {
    let message = msg.into();
    logs.push(message.clone());
    secure_logger.log(tag, serde_json::json!({ "message": message }));
}

pub fn log_to_vec(logs: &Arc<Mutex<Vec<String>>>, message: impl Into<String>) {
    if let Ok(mut lock) = logs.lock() {
        lock.push(message.into());
    }
}

fn secure_log_path() -> std::path::PathBuf {
    Path::new(LOG_DIR).join(LOG_FILE)
}

fn read_last_hash() -> String {
    let filepath = secure_log_path();
    if let Ok(content) = fs::read_to_string(&filepath) {
        content.lines().rev().find_map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .and_then(|val| val.get("current_hash")?.as_str().map(String::from))
        }).unwrap_or_default()
    } else {
        String::new()
    }
}

pub fn read_secure_log_lines() -> Vec<String> {
    let path = secure_log_path();
    if let Ok(content) = fs::read_to_string(path) {
        content
            .lines()
            .map(|line| line.to_string())
            .collect()
    } else {
        vec!["No se pudo leer secure.log o no existe.".to_string()]
    }
}

pub fn read_secure_log_formatted() -> Vec<String> {
    let path = secure_log_path();

    if let Ok(content) = fs::read_to_string(path) {
        content
            .lines()
            .filter_map(|line| {
                serde_json::from_str::<serde_json::Value>(line).ok().map(|entry| {
                    let timestamp = entry.get("timestamp").and_then(|v| v.as_str()).unwrap_or("?");
                    let event = entry.get("event").and_then(|v| v.as_str()).unwrap_or("?");
                    let details = entry.get("details")
                        .map(|d| serde_json::to_string_pretty(d).unwrap_or("{}".to_string()))
                        .unwrap_or("{}".to_string());

                    format!("[{}] {}:\n{}", timestamp, event, details)
                })
            })
            .collect()
    } else {
        vec!["No se pudo leer secure.log o no existe.".to_string()]
    }
}


pub fn validate_secure_log_integrity() -> Result<(), String> {
    let path = secure_log_path();
    let content = fs::read_to_string(path).map_err(|e| format!("Error al leer secure.log: {}", e))?;
    let mut previous_hash = String::new();

    for (line_num, line) in content.lines().enumerate() {
        let entry: serde_json::Value = serde_json::from_str(line)
            .map_err(|_| format!("Línea {} no es JSON válido", line_num + 1))?;

        // Verificamos que prev_hash coincida con el hash anterior
        let prev = entry
            .get("prev_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if prev != previous_hash {
            return Err(format!(
                "Hash inconsistente en línea {}: esperado '{}', encontrado '{}'",
                line_num + 1,
                previous_hash,
                prev
            ));
        }

        // Recalcular el hash a partir de los campos (sin current_hash)
        let content_for_hash = json!({
            "timestamp": entry.get("timestamp"),
            "event": entry.get("event"),
            "details": entry.get("details"),
            "prev_hash": entry.get("prev_hash")
        });

        let serialized = content_for_hash.to_string();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let calculated_hash = format!("{:x}", hasher.finalize());

        let current = entry
            .get("current_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if calculated_hash != current {
            return Err(format!(
                "Hash calculado no coincide en línea {}.\nEsperado: {}\nCalculado: {}",
                line_num + 1,
                current,
                calculated_hash
            ));
        }

        previous_hash = current.to_string();
    }

    Ok(())
}


