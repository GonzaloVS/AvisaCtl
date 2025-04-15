use chrono::Local;
use serde_json::json;
use std::path::Path;
use tokio::process::Command;

use crate::deploy::docker::{build_with_docker, ensure_dockerfile_exists};
use crate::deploy::logic::{extract_package_name, Platform};
use crate::securelog::logger::SecureLogger;

pub async fn run_preflight(
    project_path: &str,
    logs: &mut Vec<String>,
    _platform: &Platform,
    secure_logger: &SecureLogger,
) -> bool {
    logs.push("▶ Iniciando preflight...".to_string());

    let steps = vec![
        ("cargo fmt --check", vec!["fmt", "--", "--check"]),
        ("cargo clippy", vec!["clippy", "--", "-D", "warnings"]),
        ("cargo test", vec!["test"]),
        ("cargo audit", vec!["audit"]),
    ];

    for (name, args) in steps {
        let mut cmd = Command::new("cargo");
        cmd.args(args);
        cmd.current_dir(project_path);

        logs.push(format!("Ejecutando {}...", name));
        let output = cmd.output().await;

        match output {
            Ok(output) if output.status.success() => {
                logs.push(format!("{} superado.", name));
            }
            Ok(output) => {
                logs.push(format!("{} falló:", name));
                logs.push(String::from_utf8_lossy(&output.stderr).to_string());
                return false;
            }
            Err(e) => {
                logs.push(format!("Error ejecutando {}: {}", name, e));
                return false;
            }
        }
    }

    secure_logger.log_event(
        "preflight_dockerfile_check",
        json!({ "message": "Validación superada. Verificando Dockerfile..." }),
    );
    if !ensure_dockerfile_exists(project_path, logs, secure_logger) {
        return false;
    }

    secure_logger.log_event(
        "preflight_docker_build",
        json!({ "message": "Construyendo Docker..." }),
    );
    build_with_docker(project_path, logs, secure_logger).await
}

pub fn rename_previous_binary(
    project_path: &str,
    platform: &Platform,
    secure_logger: &SecureLogger,
) -> Option<String> {
    let pkg_name = extract_package_name(&Path::new(project_path).join("Cargo.toml"))?;
    let bin_path = Path::new(project_path)
        .join("target")
        .join("release")
        .join(match platform {
            Platform::Windows => format!("{}.exe", pkg_name),
            Platform::Linux => pkg_name.clone(),
        });

    if bin_path.exists() {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S");
        let new_path = bin_path.with_file_name(format!(
            "{} - {}{}",
            pkg_name,
            timestamp,
            if *platform == Platform::Windows {
                ".exe"
            } else {
                ""
            }
        ));

        match std::fs::rename(&bin_path, &new_path) {
            Ok(_) => {
                secure_logger.log_event(
                    "preflight_binary_renamed",
                    json!({
                        "message": format!("Binario renombrado a {}", new_path.display())
                    }),
                );
                Some(pkg_name)
            }
            Err(e) => {
                secure_logger.log_error(
                    "preflight_binary_rename_failed",
                    &format!("Error al renombrar binario: {}", e),
                );
                None
            }
        }
    } else {
        secure_logger.log_event(
            "preflight_binary_none",
            json!({
                "message": "No había binario previo que renombrar."
            }),
        );
        Some(pkg_name)
    }
}
