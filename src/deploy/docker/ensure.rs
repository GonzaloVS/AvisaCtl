use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

use crate::deploy::remote:: bin_path::extract_package_name;
use crate::securelog::logger::SecureLogger;

const DOCKERFILE_TEMPLATE: &str = include_str!("../../../assets/Dockerfile.template");

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

pub fn ensure_base_image(logs: &mut Vec<String>, secure_logger: &SecureLogger) -> bool {
    let check = StdCommand::new("docker")
        .arg("images")
        .arg("-q")
        .arg("avisactl-base")
        .output();

    match check {
        Ok(output) if !output.stdout.is_empty() => {
            logs.push("La imagen base 'avisactl-base' ya existe.".into());
            secure_logger.log_event(
                "docker_base_image_exists",
                serde_json::json!({ "image": "avisactl-base" }),
            );
            true
        }
        _ => {
            logs.push("La imagen base 'avisactl-base' no existe. Construyendo...".into());
            secure_logger.log_event(
                "docker_base_image_missing",
                serde_json::json!({ "image": "avisactl-base" }),
            );

            let dockerfile_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets")
                .join("Dockerfile.base");

            let context_path = Path::new(env!("CARGO_MANIFEST_DIR"));

            let build = StdCommand::new("docker")
                .arg("build")
                .arg("-f")
                .arg(dockerfile_path.to_string_lossy().as_ref())
                .arg("-t")
                .arg("avisactl-base")
                .arg(".")
                .current_dir(context_path)
                .output();

            match build {
                Ok(out) if out.status.success() => {
                    logs.push("Imagen base 'avisactl-base' construida con éxito.".into());
                    secure_logger.log_event(
                        "docker_base_image_built",
                        serde_json::json!({ "status": "success" }),
                    );
                    true
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    logs.push(format!("Error al construir la imagen base: {}", stderr));
                    secure_logger.log_error("docker_base_image_build_failed", &stderr);
                    false
                }
                Err(e) => {
                    logs.push(format!("Error al ejecutar docker build: {}", e));
                    secure_logger.log_error("docker_base_image_error", &e.to_string());
                    false
                }
            }
        }
    }
}