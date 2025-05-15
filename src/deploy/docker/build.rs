use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::deploy::docker::ensure::{ensure_base_image};
use crate::deploy::docker::utils::convert_windows_path_for_docker;
use crate::deploy::remote::bin_path::extract_package_name;
use crate::securelog::logger::SecureLogger;

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

    if !ensure_base_image(logs, secure_logger) {
        logs.push("No se pudo asegurar la imagen base. Abortando.".into());
        return false;
    }

    let dockerfile_name = format!("Dockerfile.{}", pkg_name);
    let base_image = "avisactl-base";
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
        .arg("--build-arg")
        .arg(format!("BASE_IMAGE={}", base_image))
        .arg("-t")
        .arg(&image_name)
        .arg(&abs_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
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
}
