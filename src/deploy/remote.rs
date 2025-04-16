use serde_json::json;
use ssh2::Session;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::config::{save_config, AvisaCtlConfig};
use crate::deploy::logic::{Platform, RemoteConfig};
use crate::deploy::preflight::rename_previous_binary;
use crate::securelog::logger::SecureLogger;

pub fn deploy_to_remote_async(
    project_path: String,
    platform: Platform,
    remote: RemoteConfig,
    mut config: AvisaCtlConfig,
    callback: impl Fn(bool) + Send + 'static,
    cancel_flag: Arc<Mutex<bool>>,
    secure_logger: Arc<SecureLogger>,
) {
    tokio::spawn(async move {
        secure_logger.log_event(
            "deploy_start",
            json!({
                "project_path": project_path,
                "server": remote.server_address,
                "user": remote.username
            }),
        );

        config.last_local_path = project_path.to_string();
        config.last_server_address = remote.server_address.clone();
        config.last_remote_user = remote.username.clone();
        config.last_remote_pass = remote.pass.clone();
        config.last_remote_path = remote.remote_path.clone();
        config.secure_log_path = remote.secure_log_path.clone();
        let _ = save_config(&config);

        let binary_name = match rename_previous_binary(&project_path, &platform, &secure_logger) {
            Some(name) => name,
            None => {
                secure_logger.log_error("deploy_fail_binary_name", "rename_previous_binary failed");
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
                &format!("No existe binario en: {}", bin_path.to_string_lossy()),
            );
            callback(false);
            return;
        }

        let remote_dest = format!(
            "{}@{}:{}",
            remote.username, remote.server_address, remote.remote_path
        );
        secure_logger.log_event(
            "deploy_scp_start",
            json!({
                "source": bin_path.to_string_lossy(),
                "destination": remote_dest
            }),
        );

        let bin_path_string = bin_path.to_string_lossy().to_string();

        // let output = Command::new("scp")
        //     .arg(bin_path_string)
        //     .arg(&remote_dest)
        //     .output()
        //     .await;

        let secure_logger_cloned = secure_logger.clone();
        let result = tokio::task::spawn_blocking({
            let remote = remote.clone();
            let bin_path_string = bin_path_string.clone();
            move || {
                upload_file_with_password(
                    &remote.server_address,
                    &remote.username,
                    &remote.pass,
                    &bin_path_string,
                    &remote.remote_path,
                    &secure_logger_cloned,
                )
            }
        })
        .await;

        if *cancel_flag.lock().unwrap() {
            secure_logger.log_event("deploy_cancelled", json!({}));
            callback(false);
            return;
        }

        match result {
            Ok(Ok(())) => {
                secure_logger.log_event("deploy_scp_success", json!({
            "destination": format!("{}@{}:{}", remote.username, remote.server_address, remote.remote_path)
        }));
                callback(true);
            }
            Ok(Err(e)) => {
                secure_logger.log_error("deploy_scp_failed", &format!("Error: {}", e));
                callback(false);
            }
            Err(join_err) => {
                secure_logger.log_error("deploy_scp_panic", &format!("Thread panic: {}", join_err));
                callback(false);
            }
        }

        // match output {
        //     Ok(output) => {
        //         if output.status.success() {
        //             secure_logger.log_event(
        //                 "deploy_scp_success",
        //                 json!({
        //                     "destination": remote_dest
        //                 }),
        //             );
        //             callback(true);
        //         } else {
        //             let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        //             secure_logger.log_error("deploy_scp_failed", &stderr);
        //             callback(false);
        //         }
        //     }
        //     Err(join_err) => {
        //         secure_logger.log_error("deploy_scp_panic", &format!("Thread panic: {}", join_err));
        //         callback(false);
        //     }
        // }

        if *cancel_flag.lock().unwrap() {
            secure_logger.log_event("deploy_cancelled", json!({}));
            callback(false);
            //return;
        }

        // match output {
        //     Ok(output) => {
        //         if output.status.success() {
        //             secure_logger.log_event("deploy_scp_success", json!({
        //                 "destination": remote_dest
        //             }));
        //             callback(true);
        //         } else {
        //             let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        //             secure_logger.log_error("deploy_scp_failed", &stderr);
        //             callback(false);
        //         }
        //     }
        //     Err(e) => {
        //         secure_logger.log_error("deploy_error_scp_exec", &e.to_string());
        //         callback(false);
        //     }
        // }
    });
}

pub fn upload_file_with_password(
    server: &str,
    username: &str,
    password: &str,
    local_path: &str,
    remote_path: &str,
    secure_logger: &SecureLogger,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // 1. Establecer conexión TCP
    //let tcp = TcpStream::connect(format!("{}:22", server))?;
    let address = format!("{}:22", server);
    let tcp = TcpStream::connect_timeout(&address.to_socket_addrs()?.next().unwrap(), std::time::Duration::from_secs(10))?;

    // 2. Crear sesión SSH
    let mut session = Session::new()?;
    session.set_tcp_stream(tcp);
    session.handshake()?;

    // 3. Autenticación con contraseña
    session.userauth_password(username, password)?;

    if !session.authenticated() {
        return Err("Falló la autenticación SSH".into());
    }

    // 4. Leer archivo local
    let mut local_file = std::fs::File::open(local_path)?;
    let metadata = local_file.metadata()?;
    let file_size = metadata.len();

    // 5. Crear archivo remoto vía SCP
    let mut remote_file = session.scp_send(Path::new(remote_path), 0o644, file_size, None)?;
    let mut buffer = Vec::new();
    local_file.read_to_end(&mut buffer)?;
    remote_file.write_all(&buffer)?;

    //eprintln!("Archivo subido correctamente a {}", remote_path);
    secure_logger.log_event(
        "upload_success",
        json!({
            "path": remote_path,
            "message": "Archivo subido correctamente"
        }),
    );
    Ok(())
}
