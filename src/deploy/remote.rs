use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use crate::config::{save_config, AvisaCtlConfig};
use crate::deploy::local::rename_previous_binary_if_exists;
use crate::deploy::logic::{Platform, RemoteConfig};

pub fn deploy_to_remote(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &Platform,
    remote: &RemoteConfig,
    config: &mut AvisaCtlConfig,
) -> bool {
    logs.push(format!(
        "Iniciando deploy a servidor: {}",
        remote.server_address
    ));

    config.last_local_path = project_path.to_string();
    config.last_server_address = remote.server_address.clone();
    config.last_remote_user = remote.username.clone();
    config.last_remote_pass = remote.pass.clone();
    config.last_remote_path = remote.remote_path.clone();
    save_config(config);

    let binary_name = match rename_previous_binary_if_exists(project_path, logs, platform) {
        Some(name) => name,
        None => {
            logs.push("Error: No se pudo determinar el binario para subir.".to_string());
            return false;
        }
    };

    let bin_path = Path::new(project_path)
        .join("target")
        .join("x86_64-unknown-linux-gnu")
        .join("release")
        .join(&binary_name);

    if !bin_path.exists() {
        logs.push("El binario no existe tras la compilación.".to_string());
        return false;
    }

    let remote_dest = format!(
        "{}@{}:{}",
        remote.username, remote.server_address, remote.remote_path
    );
    logs.push(format!("Subiendo binario a: {}", remote_dest));

    let bin_path_string = bin_path.to_string_lossy().to_string();
    let remote_dest_clone = remote_dest.clone();

    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let output = Command::new("scp")
            .arg(bin_path_string)
            .arg(&remote_dest_clone)
            .output();
        let _ = tx.send(output);
    });

    match rx.recv() {
        Ok(Ok(output)) => {
            if output.status.success() {
                logs.push("Binario subido correctamente.".to_string());
                true
            } else {
                logs.push("Falló el SCP:".to_string());
                logs.push(String::from_utf8_lossy(&output.stderr).to_string());
                false
            }
        }
        Ok(Err(e)) => {
            logs.push(format!("Error ejecutando SCP: {}", e));
            false
        }
        Err(e) => {
            logs.push(format!("Error en el hilo de SCP: {}", e));
            false
        }
    }
}
