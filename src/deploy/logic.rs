use std::fmt::format;
use std::process::Command;
use std::path::Path;
use std::fs;
use chrono::Local;

#[derive(Debug, PartialEq)]
pub enum Platform {
    Linux,
    Windows,
}

#[derive(Debug, PartialEq)]
pub enum DeployTarget {
    Remote,
    Local,
}

fn run_cargo_step(
    step_name: &str,
    command: &mut Command,
    logs: &mut Vec<String>,
    project_path: &str,
) -> bool {
    logs.push(format!("Ejecutando '{}'...", step_name));
    let output = command.current_dir(project_path).output();

    match output {
        Ok(output) => {
            if output.status.success() {
                logs.push(format!("{} completado con éxito.", step_name));
                true
            } else {
                logs.push(format!("{} falló:", step_name));
                logs.push(format!("{}", String::from_utf8_lossy(&output.stderr)));
                false
            }
        }
        Err(e) => {
            logs.push(format!("Error al ejecutar '{}': {}", step_name, e));
            false
        }
    }
}

pub fn run_pre_release_checks(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &super::logic::Platform,
) -> bool {
    logs.push("Iniciando verificaciones antes del release...".to_string());

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
        if !run_cargo_step(name, &mut command, logs, project_path) {
            logs.push("Fallo en la validación.".to_string());
            return false;
        }
    }

    logs.push("Validación completada. Procediendo al build...".to_string());

    build_with_docker(project_path, logs)
}

pub fn build_with_docker(project_path: &str, logs: &mut Vec<String>) -> bool {
    use std::process::Stdio;

    let abs_path = Path::new(project_path)
        .canonicalize()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| project_path.to_string());

    logs.push(format!("Lanzando Docker para compilar en: {}", abs_path));

    let result = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/project", abs_path))
        .arg("-w")
        .arg("/project")
        .arg("rust:latest")
        .args(["cargo", "build", "--release", "--target", "x86_64-unknown-linux-gnu"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                logs.push("Build en Docker completada con éxito.".to_string());
                true
            } else {
                logs.push("Build en Docker falló:".to_string());
                logs.push(String::from_utf8_lossy(&output.stderr).to_string());
                false
            }
        }
        Err(e) => {
            logs.push(format!("Error al ejecutar Docker: {}", e));
            false
        }
    }
}


pub fn build_project(path: &str, logs: &mut Vec<String>, platform: &Platform) -> bool {
    logs.push(format!("Ejecutando build en: {}", path));

    let path_obj = Path::new(path);
    if !path_obj.join("Cargo.toml").exists() {
        logs.push("No se encontró Cargo.toml en esa ruta.".to_string());
        return false;
    }

    // Selección de target según plataforma
    let target = match platform {
        Platform::Windows => "x86_64-pc-windows-gnu",
        Platform::Linux => "x86_64-unknown-linux-gnu",
    };

    logs.push(format!("Target de compilación: {}", target));

    let result = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("--target")
        .arg(target)
        .current_dir(path)
        .output();

    match result {
        Ok(output) => {
            if output.status.success() {
                logs.push("✅ Build completado con éxito.".to_string());
                true
            } else {
                logs.push("❌ Falló el build:".to_string());
                logs.push(format!("{}", String::from_utf8_lossy(&output.stderr)));
                false
            }
        }
        Err(err) => {
            logs.push(format!("❌ Error al ejecutar cargo: {}", err));
            false
        }
    }
}

/// Renombra el binario si ya existe (añade timestamp).
pub fn rename_previous_binary_if_exists(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &super::logic::Platform,
) -> Option<String> {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            logs.push("❌ No se pudo leer el nombre del paquete.".to_string());
            return None;
        }
    };

    let bin_path = match platform {
        super::logic::Platform::Windows => Path::new(project_path)
            .join("target")
            .join("release")
            .join(format!("{}.exe", pkg_name)),
        super::logic::Platform::Linux => Path::new(project_path)
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
            if *platform == super::logic::Platform::Windows {
                ".exe"
            } else {
                ""
            }
        );
        let new_path = bin_path.with_file_name(new_name.clone());

        if let Err(e) = fs::rename(&bin_path, &new_path) {
            logs.push(format!("❌ Error al renombrar binario previo: {}", e));
            return None;
        }

        logs.push(format!("📁 Binario anterior renombrado como: {}", new_path.display()));
    } else {
        logs.push("ℹ️ No había binario anterior que renombrar.".to_string());
    }

    Some(pkg_name)
}

/// Extrae el nombre del paquete desde Cargo.toml (por ejemplo: avisactl)
fn extract_package_name(cargo_toml_path: &Path) -> Option<String> {
    let contents = fs::read_to_string(cargo_toml_path).ok()?;
    let mut inside_package = false;

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("[package]") {
            inside_package = true;
        } else if inside_package && line.starts_with("name") {
            return line
                .split('=')
                .nth(1)
                .map(|s| s.trim().trim_matches('"').to_string());
        }
    }

    None
}