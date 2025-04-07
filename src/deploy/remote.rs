use std::path::Path;
use std::sync::{Arc, Mutex};
use serde_json::json;
use tokio::process::Command;

use crate::config::{save_config, AvisaCtlConfig};
use crate::deploy::local::rename_previous_binary_if_exists;
use crate::deploy::logic::{Platform, RemoteConfig};
use crate::helper::logger_secure::SecureLogger;

pub fn deploy_to_remote_async(
    project_path: String,
    logs: Arc<Mutex<Vec<String>>>,
    platform: Platform,
    remote: RemoteConfig,
    mut config: AvisaCtlConfig,
    callback: impl Fn(bool) + Send + 'static,
    cancel_flag: Arc<Mutex<bool>>,
    secure_logger: Arc<SecureLogger>,
) {
    tokio::spawn(async move {
        secure_logger.log("deploy_start", json!({
            "project_path": project_path,
            "server": remote.server_address,
            "user": remote.username
        }));

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
            &secure_logger,
        ) {
            Some(name) => name,
            None => {
                secure_logger.log("deploy_fail_binary_name", json!({
                    "reason": "rename_previous_binary_if_exists failed"
                }));
                callback(false);
                return;
            }
        };

        if *cancel_flag.lock().unwrap() {
            secure_logger.log("deploy_cancelled", json!({}));
            callback(false);
            return;
        }

        let bin_path = Path::new(&project_path)
            .join("target")
            .join("x86_64-unknown-linux-gnu")
            .join("release")
            .join(&binary_name);

        if !bin_path.exists() {
            secure_logger.log("deploy_fail_no_binary", json!({
                "path": bin_path.to_string_lossy()
            }));
            callback(false);
            return;
        }

        let remote_dest = format!(
            "{}@{}:{}",
            remote.username, remote.server_address, remote.remote_path
        );
        secure_logger.log("deploy_scp_start", json!({
            "source": bin_path.to_string_lossy(),
            "destination": remote_dest
        }));

        let bin_path_string = bin_path.to_string_lossy().to_string();

        let output = Command::new("scp")
            .arg(bin_path_string)
            .arg(&remote_dest)
            .output()
            .await;

        if *cancel_flag.lock().unwrap() {
            secure_logger.log("deploy_cancelled", json!({}));
            callback(false);
            return;
        }

        match output {
            Ok(output) => {
                if output.status.success() {
                    secure_logger.log("deploy_scp_success", json!({
                        "destination": remote_dest
                    }));
                    callback(true);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    secure_logger.log("deploy_scp_failed", json!({
                        "stderr": stderr
                    }));
                    callback(false);
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                secure_logger.log("deploy_error_scp_exec", json!({
                    "error": err_str
                }));
                callback(false);
            }
        }
    });
}
