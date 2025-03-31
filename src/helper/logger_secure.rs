use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
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
            last_hash: Arc::new(Mutex::new(String::new())),
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
