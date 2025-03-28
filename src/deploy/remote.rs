use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::config::{save_config, AvisaCtlConfig};
use crate::deploy::local::rename_previous_binary_if_exists;
use crate::deploy::logic::{Platform, RemoteConfig};

pub fn deploy_to_remote_async(
    project_path: String,
    logs: Arc<Mutex<Vec<String>>>,
    platform: Platform,
    remote: RemoteConfig,
    mut config: AvisaCtlConfig,
    callback: impl Fn(bool) + Send + 'static,
    cancel_flag: Arc<Mutex<bool>>,
) {
    thread::spawn(move || {
        {
            let mut logs = logs.lock().unwrap();
            logs.push(format!(
                "Iniciando deploy a servidor: {}",
                remote.server_address
            ));
        }

        config.last_local_path = project_path.to_string();
        config.last_server_address = remote.server_address.clone();
        config.last_remote_user = remote.username.clone();
        config.last_remote_pass = remote.pass.clone();
        config.last_remote_path = remote.remote_path.clone();
        save_config(&config);

        let binary_name = match rename_previous_binary_if_exists(
            &project_path,
            &mut logs.lock().unwrap(),
            &platform,
        ) {
            Some(name) => name,
            None => {
                logs.lock()
                    .unwrap()
                    .push("Error: No se pudo determinar el binario para subir.".to_string());
                callback(false);
                return;
            }
        };

        if *cancel_flag.lock().unwrap() {
            logs.lock()
                .unwrap()
                .push("Deploy cancelado por el usuario.".into());
            callback(false);
            return;
        }

        let bin_path = Path::new(&project_path)
            .join("target")
            .join("x86_64-unknown-linux-gnu")
            .join("release")
            .join(&binary_name);

        if !bin_path.exists() {
            logs.lock()
                .unwrap()
                .push("El binario no existe tras la compilación.".to_string());
            callback(false);
            return;
        }

        let remote_dest = format!(
            "{}@{}:{}",
            remote.username, remote.server_address, remote.remote_path
        );
        logs.lock()
            .unwrap()
            .push(format!("Subiendo binario a: {}", remote_dest));

        let bin_path_string = bin_path.to_string_lossy().to_string();

        let output = Command::new("scp")
            .arg(bin_path_string)
            .arg(&remote_dest)
            .output();

        if *cancel_flag.lock().unwrap() {
            logs.lock()
                .unwrap()
                .push("Deploy cancelado por el usuario.".into());
            callback(false);
            return;
        }

        match output {
            Ok(output) => {
                if output.status.success() {
                    logs.lock()
                        .unwrap()
                        .push("Binario subido correctamente.".to_string());
                    callback(true);
                } else {
                    logs.lock().unwrap().push("Falló el SCP:".to_string());
                    logs.lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&output.stderr).to_string());
                    callback(false);
                }
            }
            Err(e) => {
                logs.lock()
                    .unwrap()
                    .push(format!("Error ejecutando SCP: {}", e));
                callback(false);
            }
        }
    });
}
