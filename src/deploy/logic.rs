use chrono::Local;
use std::fs;
use std::path::Path;
use std::process::Command;

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
                logs.push(format!("{}", String::from_utf8_lossy(&output.stdout)));
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
    _platform: &Platform,
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

    logs.push("Validación completada. Verificando Dockerfile...".to_string());

    if !ensure_dockerfile_exists(project_path, logs) {
        logs.push("No se pudo crear/verificar el Dockerfile. Se cancela el build.".to_string());
        return false;
    }

    logs.push("Dockerfile verificado. Procediendo al build en Docker...".to_string());
    build_with_docker(project_path, logs)
}

pub fn build_with_docker(project_path: &str, logs: &mut Vec<String>) -> bool {
    use std::process::Stdio;

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

    // Ejecución del build dentro del contenedor
    let run_result = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!(
            "{}:/project",
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

pub fn rename_previous_binary_if_exists(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &Platform,
) -> Option<String> {
    let pkg_name = match extract_package_name(&Path::new(project_path).join("Cargo.toml")) {
        Some(name) => name,
        None => {
            logs.push("No se pudo leer el nombre del paquete.".to_string());
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
            logs.push(format!("Error al renombrar binario previo: {}", e));
            return None;
        }

        logs.push(format!(
            "Binario anterior renombrado como: {}",
            new_path.display()
        ));
    } else {
        logs.push("No había binario anterior que renombrar.".to_string());
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
