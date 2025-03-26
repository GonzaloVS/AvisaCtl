use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::deploy::logic::extract_package_name;

pub fn ensure_dockerfile_exists(project_path: &str, logs: &mut Vec<String>) -> bool {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            logs.push("No se pudo leer el nombre del paquete para generar Dockerfile.".to_string());
            return false;
        }
    };

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let dockerfile_path = Path::new(project_path).join(&dockerfile_name);

    if dockerfile_path.exists() {
        logs.push(format!("{} ya existe.", dockerfile_name));
        return true;
    }

    let dockerfile_contents = r#"
FROM rust:latest

RUN apt update && apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    curl \
    && cargo install cargo-audit

WORKDIR /project
"#;

    match fs::write(&dockerfile_path, dockerfile_contents.trim_start()) {
        Ok(_) => {
            logs.push(format!("{} generado automáticamente.", dockerfile_name));
            true
        }
        Err(e) => {
            logs.push(format!("No se pudo crear {}: {}", dockerfile_name, e));
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

pub fn build_with_docker(project_path: &str, logs: &mut Vec<String>) -> bool {
    let abs_path_buf = Path::new(project_path)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(project_path).to_path_buf());

    let abs_path = abs_path_buf.to_string_lossy().replace("\\\\?\\", "");

    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            logs.push("No se pudo leer el nombre del paquete.".to_string());
            return false;
        }
    };

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let image_name = format!("{}-build", pkg_name.to_lowercase());

    logs.push(format!("Construyendo imagen Docker '{}'", image_name));

    let build_result = Command::new("docker")
        .arg("build")
        .arg("-f")
        .arg(&dockerfile_name)
        .arg("-t")
        .arg(&image_name)
        .arg(&abs_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match build_result {
        Ok(output) => {
            if output.status.success() {
                logs.push("Imagen Docker construida correctamente.".to_string());
            } else {
                logs.push("Falló la construcción de la imagen Docker:".to_string());
                logs.push(String::from_utf8_lossy(&output.stderr).to_string());
                return false;
            }
        }
        Err(e) => {
            logs.push(format!("Error ejecutando docker build: {}", e));
            return false;
        }
    }

    logs.push("Lanzando contenedor para compilar el binario...".to_string());

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
        .output();

    match run_result {
        Ok(output) => {
            if output.status.success() {
                logs.push("Build en Docker completado con éxito.".to_string());
                true
            } else {
                logs.push("Build en Docker falló:".to_string());
                logs.push(String::from_utf8_lossy(&output.stderr).to_string());
                false
            }
        }
        Err(e) => {
            logs.push(format!("Error al ejecutar Docker run: {}", e));
            false
        }
    }
}
