use chrono::Local;
use std::fs;
use std::path::Path;
use tokio::process::Command;

use crate::deploy::docker::{build_with_docker, ensure_dockerfile_exists};
use crate::deploy::logic::{extract_package_name, Platform};
use crate::helper::logger_secure::{SecureLogger, log_both};

async fn run_cargo_step(
    step_name: &str,
    command: &mut Command,
    logs: &mut Vec<String>,
    project_path: &str,
    secure_logger: &SecureLogger,
) -> bool {
    log_both(logs, secure_logger, format!("Ejecutando '{}'...", step_name), "precheck_step_start");
    let output = command.current_dir(project_path).output().await;

    match output {
        Ok(output) => {
            if output.status.success() {
                log_both(logs, secure_logger, format!("{} completado con éxito.", step_name), "precheck_step_success");
                true
            } else {
                log_both(logs, secure_logger, format!("{} falló:", step_name), "precheck_step_failed");
                log_both(logs, secure_logger, String::from_utf8_lossy(&output.stdout), "precheck_step_stdout");
                false
            }
        }
        Err(e) => {
            log_both(logs, secure_logger, format!("Error al ejecutar '{}': {}", step_name, e), "precheck_step_error");
            false
        }
    }
}

pub async fn run_pre_release_checks(
    project_path: &str,
    logs: &mut Vec<String>,
    _platform: &Platform,
    secure_logger: &SecureLogger,
) -> bool {
    log_both(logs, secure_logger, "Iniciando verificaciones antes del release...", "precheck_start");

    let steps: Vec<(&str, Command)> = vec![
        ("cargo fmt --check", {
            let mut cmd = Command::new("cargo");
            cmd.arg("fmt").arg("--").arg("--check");
            cmd
        }),
        ("cargo clippy -- -D warnings", {
            let mut cmd = Command::new("cargo");
            cmd.arg("clippy").arg("--").arg("-D").arg("warnings");
            cmd
        }),
        ("cargo test", {
            let mut cmd = Command::new("cargo");
            cmd.arg("test");
            cmd
        }),
        ("cargo audit", {
            let mut cmd = Command::new("cargo");
            cmd.arg("audit");
            cmd
        }),
    ];

    for (name, mut command) in steps {
        if !run_cargo_step(name, &mut command, logs, project_path, secure_logger).await {
            log_both(logs, secure_logger, "Fallo en la validación.", "precheck_failed");
            return false;
        }
    }

    log_both(logs, secure_logger, "Validación completada. Verificando Dockerfile...", "precheck_validated");

    if !ensure_dockerfile_exists(project_path, logs, secure_logger) {
        log_both(
            logs,
            secure_logger,
            "No se pudo crear/verificar el Dockerfile. Se cancela el build.",
            "dockerfile_verification_failed",
        );
        return false;
    }

    log_both(
        logs,
        secure_logger,
        "Dockerfile verificado. Procediendo al build en Docker...",
        "dockerfile_verified",
    );

    build_with_docker(project_path, logs, secure_logger).await
}

pub fn rename_previous_binary_if_exists(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &Platform,
    secure_logger: &SecureLogger,
) -> Option<String> {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            log_both(logs, secure_logger, "No se pudo leer el nombre del paquete.", "rename_package_failed");
            return None;
        }
    };

    let bin_path = match platform {
        Platform::Windows => Path::new(project_path)
            .join("target")
            .join("release")
            .join(format!("{}.exe", pkg_name)),
        Platform::Linux => Path::new(project_path)
            .join("target")
            .join("release")
            .join(&pkg_name),
    };

    if bin_path.exists() {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let new_name = format!(
            "{} - {}{}",
            pkg_name,
            timestamp,
            if *platform == Platform::Windows {
                ".exe"
            } else {
                ""
            }
        );
        let new_path = bin_path.with_file_name(new_name.clone());

        if let Err(e) = fs::rename(&bin_path, &new_path) {
            log_both(logs, secure_logger, format!("Error al renombrar binario previo: {}", e), "rename_binary_failed");
            return None;
        }

        log_both(
            logs,
            secure_logger,
            format!("Binario anterior renombrado como: {}", new_path.display()),
            "rename_binary_success",
        );
    } else {
        log_both(logs, secure_logger, "No había binario anterior que renombrar.", "rename_binary_none");
    }

    Some(pkg_name)
}
