use crate::securelog::entry::SecureLogEntry;
use crate::securelog::logger::SecureLogger;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub fn validate_secure_log_integrity_path(path: &Path) -> Result<(), String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Error al leer secure.log: {}", e))?;
    let mut previous_hash = String::new();

    for (line_num, line) in content.lines().enumerate() {
        let entry: SecureLogEntry = serde_json::from_str(line)
            .map_err(|_| format!("Línea {} no es JSON válido", line_num + 1))?;

        if entry.prev_hash != previous_hash {
            return Err(format!(
                "Hash inconsistente en línea {}: esperado '{}', encontrado '{}'",
                line_num + 1,
                previous_hash,
                entry.prev_hash
            ));
        }

        let content_for_hash = json!({
            "timestamp": entry.timestamp,
            "event": entry.event,
            "details": entry.details,
            "prev_hash": entry.prev_hash
        });

        let serialized = content_for_hash.to_string();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let calculated_hash = format!("{:x}", hasher.finalize());

        if calculated_hash != entry.current_hash {
            return Err(format!(
                "Hash calculado no coincide en línea {}.\nEsperado: {}\nCalculado: {}",
                line_num + 1,
                entry.current_hash,
                calculated_hash
            ));
        }

        previous_hash = entry.current_hash;
    }

    Ok(())
}

/// Crea el secure.log con entrada inicial si aún no existe
pub fn ensure_secure_log_initialized_at(folder: &str) -> Result<(), String> {
    let mut path = PathBuf::from(folder);
    path.push("secure.log");

    if !path.exists() {
        SecureLogger::new_with_path(&path)
            .log("secure.log inicializado", json!({}))
            .map_err(|e| format!("Fallo al inicializar secure.log: {e}"))?;
    }

    Ok(())
}

/// Valida la integridad de un secure.log desde una carpeta específica
pub fn validate_secure_log_integrity_at(folder: &str) -> Result<(), String> {
    let mut path = PathBuf::from(folder);
    path.push("secure.log");

    crate::securelog::validate_secure_log_integrity_path(&path)
}
