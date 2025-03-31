use chrono::Local;
use std::fs;
use std::path::Path;
use tokio::process::Command;

use crate::deploy::docker::build_with_docker;
use crate::deploy::docker::ensure_dockerfile_exists;
use crate::deploy::logic::{extract_package_name, Platform};
use crate::helper::logger_secure::log;

async fn run_cargo_step(
    step_name: &str,
    command: &mut Command,
    logs: &mut Vec<String>,
    project_path: &str,
) -> bool {
    log(logs, format!("Ejecutando '{}'...", step_name));
    let output = command.current_dir(project_path).output().await;

    match output {
        Ok(output) => {
            if output.status.success() {
                log(logs, format!("{} completado con éxito.", step_name));
                true
            } else {
                log(logs, format!("{} falló:", step_name));
                log(logs, String::from_utf8_lossy(&output.stdout));
                false
            }
        }
        Err(e) => {
            log(logs, format!("Error al ejecutar '{}': {}", step_name, e));
            false
        }
    }
}

pub async fn run_pre_release_checks(
    project_path: &str,
    logs: &mut Vec<String>,
    _platform: &Platform,
) -> bool {
    log(logs, "Iniciando verificaciones antes del release...");

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
        if !run_cargo_step(name, &mut command, logs, project_path).await {
            log(logs, "Fallo en la validación.");
            return false;
        }
    }

    log(logs, "Validación completada. Verificando Dockerfile...");

    if !ensure_dockerfile_exists(project_path, logs) {
        log(
            logs,
            "No se pudo crear/verificar el Dockerfile. Se cancela el build.",
        );
        return false;
    }

    log(
        logs,
        "Dockerfile verificado. Procediendo al build en Docker...",
    );
    build_with_docker(project_path, logs).await
}

pub fn rename_previous_binary_if_exists(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &Platform,
) -> Option<String> {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            log(logs, "No se pudo leer el nombre del paquete.");
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
            log(logs, format!("Error al renombrar binario previo: {}", e));
            return None;
        }

        log(
            logs,
            format!("Binario anterior renombrado como: {}", new_path.display()),
        );
    } else {
        log(logs, "No había binario anterior que renombrar.");
    }

    Some(pkg_name)
}
