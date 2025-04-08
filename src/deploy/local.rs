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
) -> bool {
    logs.push(format!("Ejecutando {}...", step_name));
    let output = command.output().await;

    match output {
        Ok(output) => {
            if output.status.success() {
                logs.push(format!("{} superado.", step_name));
                true
            } else {
                logs.push(format!("{} falló:", step_name));
                logs.push(String::from_utf8_lossy(&output.stderr).to_string());
                false
            }
        }
        Err(e) => {
            logs.push(format!("Error ejecutando {}: {}", step_name, e));
            false
        }
    }
}

pub async fn run_pre_release_checks(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &Platform,
    secure_logger: &SecureLogger,
) -> bool {
    logs.push("▶ Iniciando validaciones...".to_string());

    let steps: Vec<(&str, Command)> = vec![
        ("cargo fmt --check", {
            let mut cmd = Command::new("cargo");
            cmd.arg("fmt").arg("--").arg("--check");
            cmd.current_dir(project_path);
            cmd
        }),
        ("cargo clippy", {
            let mut cmd = Command::new("cargo");
            cmd.arg("clippy").arg("--").arg("-D").arg("warnings");
            cmd.current_dir(project_path);
            cmd
        }),
        ("cargo test", {
            let mut cmd = Command::new("cargo");
            cmd.arg("test");
            cmd.current_dir(project_path);
            cmd
        }),
        ("cargo audit", {
            let mut cmd = Command::new("cargo");
            cmd.arg("audit");
            cmd.current_dir(project_path);
            cmd
        }),
    ];

    for (name, mut command) in steps {
        if !run_cargo_step(name, &mut command, logs).await {
            logs.push("Cancelando proceso por error en validación.".to_string());
            return false;
        }
    }

    logs.push("Validación completada. Verificando Dockerfile...".to_string());

    if !ensure_dockerfile_exists(project_path, logs) {
        logs.push("No se pudo verificar/crear el Dockerfile.".to_string());
        return false;
    }

    logs.push("Dockerfile verificado. Construyendo con Docker...".to_string());

    if build_with_docker(project_path, logs).await {
        logs.push("Build Docker completado.".to_string());
        true
    } else {
        logs.push("Build Docker falló.".to_string());
        false
    }
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
