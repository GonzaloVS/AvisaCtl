use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::config::{save_config, AvisaCtlConfig};
use crate::deploy::logic::Platform;
use crate::deploy::remote::bin_path::extract_package_name;
use crate::deploy::remote::config::RemoteConfig;

use crate::deploy::remote_uploader::RemoteUploader;
use crate::securelog::logger::SecureLogger;
use serde_json::json;
use tokio::task;

pub fn deploy_to_remote_async(
    project_path: String,
    platform: Platform,
    remote: RemoteConfig,
    mut config: AvisaCtlConfig,
    callback: impl Fn(bool) + Send + 'static,
    cancel_flag: Arc<Mutex<bool>>,
    secure_logger: Arc<SecureLogger>,
) {
    task::spawn(async move {
        secure_logger.log_event(
            "deploy_start",
            json!({
                "project_path": project_path,
                "server": remote.server_address,
                "user": remote.username
            }),
        );

        config.last_local_path = project_path.clone();
        config.last_server_address = remote.server_address.clone();
        config.last_remote_user = remote.username.clone();
        config.last_remote_pass = remote.pass.clone();
        config.last_remote_path = remote.remote_path.clone();
        config.secure_log_path = remote.secure_log_path.clone();
        let _ = save_config(&config);

        let binary_name = match extract_package_name(&Path::new(&project_path).join("Cargo.toml")) {
            Some(name) => match platform {
                //Platform::Windows => format!("{name}.exe"),
                Platform::Linux => name,
            },
            None => {
                secure_logger.log_error("deploy_fail_binary_name", "No se pudo leer el nombre del paquete desde Cargo.toml");
                callback(false);
                return;
            }
        };

        if *cancel_flag.lock().unwrap() {
            secure_logger.log_event("deploy_cancelled", json!({}));
            callback(false);
            return;
        }

        let bin_path = Path::new(&project_path)
            .join("target")
            .join("x86_64-unknown-linux-gnu")
            .join("release")
            .join(&binary_name);

        if !bin_path.exists() {
            secure_logger.log_error(
                "deploy_fail_no_binary",
                &format!("No existe binario en: {}", bin_path.display()),
            );
            callback(false);
            return;
        }

        let uploader = RemoteUploader::new(remote.clone(), secure_logger.clone());

        if *cancel_flag.lock().unwrap() {
            secure_logger.log_event("deploy_cancelled", json!({}));
            callback(false);
            return;
        }

        match uploader.upload_binary(&bin_path).await {
            Ok(_) => {
                secure_logger.log_event("deploy_completed", json!({}));
                callback(true);
            }
            Err(e) => {
                secure_logger.log_error("deploy_failed", &format!("Error subiendo: {}", e));
                callback(false);
            }
        }
    });
}
