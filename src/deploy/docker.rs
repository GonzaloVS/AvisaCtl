use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::deploy::logic::extract_package_name;
use crate::securelog::logger::SecureLogger;

const DOCKERFILE_TEMPLATE: &str = include_str!("../assets/Dockerfile.template");

pub fn ensure_dockerfile_exists(
    project_path: &str,
    logs: &mut Vec<String>,
    secure_logger: &SecureLogger,
) -> bool {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            logs.push("No se pudo leer el nombre del paquete.".into());
            secure_logger.log_error(
                "dockerfile_package_error",
                "No se pudo leer el nombre del paquete para generar Dockerfile.",
            );
            return false;
        }
    };

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let dockerfile_path = Path::new(project_path).join(&dockerfile_name);

    if dockerfile_path.exists() {
        logs.push(format!("{} ya existe.", dockerfile_name));
        secure_logger.log_event(
            "dockerfile_exists",
            serde_json::json!({ "path": dockerfile_path }),
        );
        return true;
    }

    match fs::write(&dockerfile_path, DOCKERFILE_TEMPLATE.trim_start()) {
        Ok(_) => {
            logs.push(format!("{} generado automáticamente.", dockerfile_name));
            secure_logger.log_event(
                "dockerfile_created",
                serde_json::json!({ "path": dockerfile_path }),
            );
            true
        }
        Err(e) => {
            logs.push(format!("No se pudo crear {}: {}", dockerfile_name, e));
            secure_logger.log_error(
                "dockerfile_creation_failed",
                &format!("{}: {}", dockerfile_name, e),
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
            logs.push("No se pudo leer el nombre del paquete.".into());
            secure_logger.log_error(
                "docker_package_error",
                "No se pudo leer el nombre del paquete.",
            );
            return false;
        }
    };

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let image_name = format!("{}-build", pkg_name.to_lowercase());

    logs.push(format!("Construyendo imagen Docker '{}'", image_name));
    secure_logger.log_event(
        "docker_build_start",
        serde_json::json!({
            "image_name": image_name,
            "dockerfile": dockerfile_name
        }),
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
                logs.push("Imagen Docker construida correctamente.".into());
                secure_logger.log_event(
                    "docker_build_success",
                    serde_json::json!({ "image_name": image_name }),
                );
            } else {
                logs.push("Falló la construcción de la imagen Docker.".into());
                secure_logger.log_error(
                    "docker_build_failed",
                    &String::from_utf8_lossy(&output.stderr),
                );
                return false;
            }
        }
        Err(e) => {
            logs.push(format!("Error ejecutando docker build: {}", e));
            secure_logger.log_error("docker_build_error", &e.to_string());
            return false;
        }
    }

    logs.push("Lanzando contenedor para compilar el binario...".into());
    secure_logger.log_event(
        "docker_run_start",
        serde_json::json!({ "image_name": image_name }),
    );

    let mut child = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!(
            "{}/:/project",
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
        .spawn()
        .expect("Docker build failed");

    let stdout = child.stdout.take().expect("No se pudo capturar stdout");
    let stderr = child.stderr.take().expect("No se pudo capturar stderr");

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut logs_out = Vec::new();
    let stdout_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stdout_reader.next_line().await {
            logs_out.push(format!("STDOUT: {}", line));
        }
        logs_out
    });

    let mut logs_err = Vec::new();
    let stderr_task = tokio::spawn(async move {
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            logs_err.push(format!("STDERR: {}", line));
        }
        logs_err
    });

    let status = child.wait().await.expect("Fallo esperando docker run");

    let stdout_lines = stdout_task.await.unwrap_or_default();
    let stderr_lines = stderr_task.await.unwrap_or_default();

    logs.extend(stdout_lines);
    logs.extend(stderr_lines);

    if status.success() {
        logs.push("Build en Docker completado con éxito.".into());
        secure_logger.log_event(
            "docker_run_success",
            serde_json::json!({ "image": image_name }),
        );
        true
    } else {
        logs.push("Build en Docker falló.".into());
        secure_logger.log_error(
            "docker_run_failed",
            "Error al compilar en contenedor Docker.",
        );
        false
    }


    // let run_result = Command::new("docker")
    //     .arg("run")
    //     .arg("--rm")
    //     .arg("-v")
    //     .arg(format!(
    //         "{}/:/project",
    //         convert_windows_path_for_docker(&abs_path)
    //     ))
    //     .arg("-v")
    //     .arg(format!(
    //         "{}/target:/project/target",
    //         convert_windows_path_for_docker(&abs_path)
    //     ))
    //     .arg("-w")
    //     .arg("/project")
    //     .arg(&image_name)
    //     .args([
    //         "cargo",
    //         "build",
    //         "--release",
    //         "--target",
    //         "x86_64-unknown-linux-gnu",
    //     ])
    //     .stdout(Stdio::piped())
    //     .stderr(Stdio::piped())
    //     .output()
    //     .await;
    //
    // match run_result {
    //     Ok(output) => {
    //         if output.status.success() {
    //             logs.push("Build en Docker completado con éxito.".into());
    //             secure_logger.log_event(
    //                 "docker_run_success",
    //                 serde_json::json!({ "image": image_name }),
    //             );
    //             true
    //         } else {
    //             logs.push("Build en Docker falló.".into());
    //             secure_logger.log_error(
    //                 "docker_run_failed",
    //                 &String::from_utf8_lossy(&output.stderr),
    //             );
    //             false
    //         }
    //     }
    //     Err(e) => {
    //         logs.push(format!("Error al ejecutar Docker run: {}", e));
    //         secure_logger.log_error("docker_run_error", &e.to_string());
    //         false
    //     }
    // }
}
