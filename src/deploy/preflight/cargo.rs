use crate::securelog::logger::SecureLogger;
use serde_json::json;
use std::process::Stdio;
use tokio::process::Command;

pub async fn run_cargo_steps(
    project_path: &str,
    logs: &mut Vec<String>,
    logger: &SecureLogger,
    fmt: bool,
    clippy: bool,
    test: bool,
    audit: bool,
) -> bool {
    let mut steps = Vec::new();

    if fmt {
        steps.push(("cargo fmt --check", vec!["fmt", "--", "--check"]));
    }
    if clippy {
        steps.push(("cargo clippy", vec!["clippy", "--", "-D", "warnings"]));
    }
    if test {
        steps.push(("cargo test", vec!["test"]));
    }
    if audit {
        steps.push(("cargo audit", vec!["audit"]));
    }

    for (name, args) in steps {
        logs.push(format!("Ejecutando {}...", name));

        let output = Command::new("cargo")
            .args(args)
            .current_dir(project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                logs.push(format!("{} superado.", name));
            }
            Ok(out) => {
                logs.push(format!("{} falló:", name));
                logs.push(String::from_utf8_lossy(&out.stderr).to_string());
                return false;
            }
            Err(e) => {
                logs.push(format!("Error ejecutando {}: {}", name, e));
                return false;
            }
        }
    }

    logger.log_event("preflight_cargo_ok", json!({ "message": "Validaciones cargo completadas." }));
    true
}
