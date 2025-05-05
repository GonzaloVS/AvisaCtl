use crate::checks::bin_size::{check_binary_size, MAX_SIZE_BYTES};
use crate::checks::lockfile::{check_lockfile, LockfileCheck};
use crate::checks::unwraps::check_no_dangerous_patterns;
use crate::deploy::logic::Platform;
use crate::securelog::logger::SecureLogger;
use serde_json::json;
use std::path::Path;

pub fn run_unwrap_check(
    project_path: &str,
    logs: &mut Vec<String>,
    logger: &SecureLogger,
) -> bool {
    logs.push("Comprobando uso peligroso de unwrap/expect...".to_string());
    match check_no_dangerous_patterns(Path::new(project_path)) {
        Ok(_) => {
            logs.push("Sin unwraps/expect peligrosos.".to_string());
            logger.log_event("preflight_unwrap_check", json!({
                "status": "ok", "message": "No se detectaron unwraps peligrosos."
            }));
            true
        }
        Err(errors) => {
            logs.push("Detectado uso peligroso de unwrap/expect:".to_string());
            for e in &errors {
                logs.push(format!(" - {}", e));
            }
            logger.log_error(
                "preflight_unwrap_check_failed",
                &format!("Se detectaron usos peligrosos:\n{}", errors.join("\n")),
            );
            false
        }
    }
}

pub fn run_lockfile_check(
    project_path: &str,
    logs: &mut Vec<String>,
    logger: &SecureLogger,
) -> bool {
    logs.push("Verificando Cargo.lock...".to_string());
    match check_lockfile(Path::new(project_path)) {
        Ok(result) => match result.status {
            LockfileCheck::Missing => {
                logs.push(format!(
                    "Cargo.lock no existe en {}.",
                    result.lockfile_path.display()
                ));
                logger.log_error(
                    "preflight_lockfile_missing",
                    &format!("Falta el archivo Cargo.lock en {}. Ejecuta 'cargo build' para generarlo.",
                             result.lockfile_path.display()
                    ),
                );
                false
            }
            LockfileCheck::Outdated => {
                logs.push(format!(
                    "Cargo.lock desactualizado en {}.",
                    result.lockfile_path.display()
                ));
                logger.log_error(
                    "preflight_lockfile_outdated",
                    &format!(
                        "El archivo Cargo.lock en {} está desactualizado. Ejecuta 'cargo update'.",
                        result.lockfile_path.display()
                    ),
                );
                false
            }
            LockfileCheck::DirtyGitState => {
                logs.push(format!(
                    "Cargo.lock fue modificado manualmente o tiene cambios sin commitear: {}",
                    result.lockfile_path.display()
                ));
                logger.log_event("preflight_lockfile_dirty", json!({
                    "message": format!(
                        "Cargo.lock con cambios detectados: {}",
                        result.lockfile_path.display()
                    )
                }));
                true
            }
            LockfileCheck::Ok => {
                logs.push(format!(
                    "Cargo.lock verificado correctamente en {}.",
                    result.lockfile_path.display()
                ));
                logger.log_event("preflight_lockfile_ok", json!({
                    "message": format!(
                        "Cargo.lock válido y sincronizado: {}",
                        result.lockfile_path.display()
                    )
                }));
                true
            }
        },
        Err(e) => {
            logs.push(format!("Error verificando Cargo.lock: {}", e));
            logger.log_error("preflight_lockfile_error", &e);
            false
        }
    }
}

pub fn run_bin_size_check(
    project_path: &str,
    platform: &Platform,
    max_size: u64,
    logs: &mut Vec<String>,
    logger: &SecureLogger,
) -> bool {
    logs.push("Verificando tamaño del binario...".to_string());
    match check_binary_size(Path::new(project_path), platform, Some(max_size)) {
        Ok(result) if !result.exists => {
            logs.push("No se encontró binario release. ¿Ejecutaste cargo build --release?".to_string());
            logger.log_error(
                "preflight_binary_missing",
                "El binario release no fue encontrado en target/release/",
            );
            false
        }
        Ok(result) if result.too_large => {
            logs.push(format!(
                "El binario es demasiado grande: {:.2} MB (límite: {:.2} MB)",
                result.bin_size_bytes as f64 / 1_048_576.0,
                MAX_SIZE_BYTES as f64 / 1_048_576.0
            ));
            logger.log_error(
                "preflight_binary_too_large",
                &format!(
                    "Tamaño: {} bytes. Ruta: {}",
                    result.bin_size_bytes,
                    result.bin_path.display()
                ),
            );
            false
        }
        Ok(result) => {
            logs.push(format!(
                "✓ Binario correcto: {:.2} MB",
                result.bin_size_bytes as f64 / 1_048_576.0
            ));
            logger.log_event(
                "preflight_binary_size_ok",
                json!({
                    "path": result.bin_path,
                    "size_bytes": result.bin_size_bytes
                }),
            );
            true
        }
        Err(e) => {
            logs.push(format!("Error revisando el binario: {}", e));
            logger.log_error("preflight_binary_check_failed", &e);
            false
        }
    }
}
