use crate::securelog::crypto::{fetch_tsa_timestamp, sign_with_gpg};
use crate::securelog::entry::SecureLogEntry;
use crate::securelog::file::{read_last_hash, resolve_log_path};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct SecureLogger {
    last_hash: Arc<Mutex<String>>,
    log_path: std::path::PathBuf,
}

impl SecureLogger {
    pub fn new() -> Self {
        let log_path = resolve_log_path();
        fs::create_dir_all(log_path.parent().unwrap()).unwrap();
        initialize_secure_log_if_needed(&log_path);
        let last_hash = read_last_hash(&log_path);
        Self {
            last_hash: Arc::new(Mutex::new(last_hash)),
            log_path,
        }
    }

    pub fn new_with_path(path: &Path) -> Self {
        fs::create_dir_all(path.parent().unwrap())
            .expect("No se pudo crear directorio para secure.log");
        initialize_secure_log_if_needed(path);
        let last_hash = read_last_hash(path);
        Self {
            last_hash: Arc::new(Mutex::new(last_hash)),
            log_path: path.to_path_buf(),
        }
    }

    pub fn log(
        &self,
        event: &str,
        details: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let now = chrono::Utc::now().to_rfc3339();
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

        let full_entry = SecureLogEntry {
            timestamp: now,
            event: event.to_string(),
            details,
            prev_hash,
            current_hash: current_hash.clone(),
            gpg_signature: sign_with_gpg(&current_hash).ok(),
            tsa_timestamp: fetch_tsa_timestamp(&current_hash).ok(),
        };

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| format!("No se pudo abrir secure.log: {e}"))?;

        writeln!(file, "{}", serde_json::to_string(&full_entry)?)?;
        Ok(())
    }

    pub fn log_event(&self, tag: &str, payload: serde_json::Value) {
        if let Err(e) = self.log(
            tag,
            json!({
            "level": "info",
            "payload": payload
        }),
        ) {
            eprintln!("Fallo en log_event: {e}");
        }
    }

    pub fn log_error(&self, tag: &str, description: &str) {
        if let Err(e) = self.log(
            tag,
            json!({
            "level": "error",
            "message": description
        }),
        ) {
            eprintln!("Fallo en log_error: {e}");
        }
    }
}

fn initialize_secure_log_if_needed(path: &Path) {
    if path.exists() {
        if let Ok(contents) = fs::read_to_string(path) {
            for line in contents.lines().rev() {
                if serde_json::from_str::<SecureLogEntry>(line).is_ok() {
                    return;
                }
            }
        }
    }

    let timestamp = chrono::Utc::now().to_rfc3339();
    let content = json!({
        "timestamp": timestamp,
        "event": "secure_log_initialized",
        "details": { "message": "Inicio del secure.log" },
        "prev_hash": ""
    });

    let serialized = content.to_string();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let current_hash = format!("{:x}", hasher.finalize());

    let full_entry = SecureLogEntry {
        timestamp,
        event: "secure_log_initialized".to_string(),
        details: json!({ "message": "Inicio del secure.log" }),
        prev_hash: "".to_string(),
        current_hash: current_hash.clone(),
        gpg_signature: sign_with_gpg(&current_hash).ok(),
        tsa_timestamp: fetch_tsa_timestamp(&current_hash).ok(),
    };

    fs::create_dir_all(path.parent().unwrap()).expect("No se pudo crear directorio del log");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .expect("No se pudo crear secure.log inicial");

    writeln!(file, "{}", serde_json::to_string(&full_entry).unwrap())
        .expect("No se pudo escribir secure.log inicial");
}
