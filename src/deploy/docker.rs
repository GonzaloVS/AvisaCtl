use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::deploy::logic::extract_package_name;
use crate::helper::logger_secure::{log_both, SecureLogger};

const DOCKERFILE_TEMPLATE: &str = include_str!("../assets/Dockerfile.template");

pub fn ensure_dockerfile_exists(
    project_path: &str,
    logs: &mut Vec<String>,
    secure_logger: &SecureLogger,
) -> bool {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            log_both(
                logs,
                secure_logger,
                "No se pudo leer el nombre del paquete para generar Dockerfile.",
                "dockerfile_package_error",
            );
            return false;
        }
    };

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let dockerfile_path = Path::new(project_path).join(&dockerfile_name);

    if dockerfile_path.exists() {
        log_both(
            logs,
            secure_logger,
            format!("{} ya existe.", dockerfile_name),
            "dockerfile_exists",
        );
        return true;
    }

    match fs::write(&dockerfile_path, DOCKERFILE_TEMPLATE.trim_start()) {
        Ok(_) => {
            log_both(
                logs,
                secure_logger,
                format!("{} generado automáticamente.", dockerfile_name),
                "dockerfile_created",
            );
            true
        }
        Err(e) => {
            log_both(
                logs,
                secure_logger,
                format!("No se pudo crear {}: {}", dockerfile_name, e),
                "dockerfile_creation_failed",
            );
            false
        }
    }
}

/// Convierte rutas tipo `C:\...` a `/c/...` en Windows. En Linux no modifica nada.
#[cfg(target_os = "windows")]
fn convert_windows_path_for_docker(path: &str) -> String {
    let drive_letter = &path[0..1].to_lowercase();
    let without_colon = path[2..].replace("\\", "/");
    format!("/{}/{}", drive_letter, without_colon)
}

#[cfg(not(target_os = "windows"))]
fn convert_windows_path_for_docker(path: &str) -> String {
    path.to_string()
}

pub async fn build_with_docker(
    project_path: &str,
    logs: &mut Vec<String>,
    secure_logger: &SecureLogger,
) -> bool {
    let abs_path_buf = Path::new(project_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(project_path).to_path_buf());

    let abs_path = abs_path_buf.to_string_lossy().replace("\\\\?\\", "");

    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            log_both(logs, secure_logger, "No se pudo leer el nombre del paquete.", "docker_package_error");
            return false;
        }
    };

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let image_name = format!("{}-build", pkg_name.to_lowercase());

    log_both(
        logs,
        secure_logger,
        format!("Construyendo imagen Docker '{}'", image_name),
        "docker_build_start",
    );

    let build_result = Command::new("docker")
        .arg("build")
        .arg("-f")
        .arg(&dockerfile_name)
        .arg("-t")
        .arg(&image_name)
        .arg(&abs_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match build_result {
        Ok(output) => {
            if output.status.success() {
                log_both(logs, secure_logger, "Imagen Docker construida correctamente.", "docker_build_success");
            } else {
                log_both(logs, secure_logger, "Falló la construcción de la imagen Docker:", "docker_build_failed");
                log_both(logs, secure_logger, String::from_utf8_lossy(&output.stderr), "docker_build_stderr");
                return false;
            }
        }
        Err(e) => {
            log_both(logs, secure_logger, format!("Error ejecutando docker build: {}", e), "docker_build_error");
            return false;
        }
    }

    log_both(
        logs,
        secure_logger,
        "Lanzando contenedor para compilar el binario...",
        "docker_run_start",
    );

    let run_result = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!(
            "{}:/project",
            convert_windows_path_for_docker(&abs_path)
        ))
        .arg("-v")
        .arg(format!(
            "{}/target:/project/target",
            convert_windows_path_for_docker(&abs_path)
        ))
        .arg("-w")
        .arg("/project")
        .arg(&image_name)
        .args([
            "cargo",
            "build",
            "--release",
            "--target",
            "x86_64-unknown-linux-gnu",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match run_result {
        Ok(output) => {
            if output.status.success() {
                log_both(logs, secure_logger, "Build en Docker completado con éxito.", "docker_run_success");
                true
            } else {
                log_both(logs, secure_logger, "Build en Docker falló:", "docker_run_failed");
                log_both(logs, secure_logger, String::from_utf8_lossy(&output.stderr), "docker_run_stderr");
                false
            }
        }
        Err(e) => {
            log_both(logs, secure_logger, format!("Error al ejecutar Docker run: {}", e), "docker_run_error");
            false
        }
    }
}
