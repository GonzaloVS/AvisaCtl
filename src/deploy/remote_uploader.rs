use serde_json::json;
use ssh2::Session;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;
use tokio::time::timeout;

use crate::deploy::logic::RemoteConfig;
use crate::securelog::logger::SecureLogger;

pub struct RemoteUploader {
    remote: RemoteConfig,
    secure_logger: Arc<SecureLogger>,
}

impl RemoteUploader {
    pub fn new(remote: RemoteConfig, secure_logger: Arc<SecureLogger>) -> Self {
        Self {
            remote,
            secure_logger,
        }
    }

    pub async fn upload_binary(&self, local_path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let local_path_string = local_path.to_string_lossy().to_string();

        for attempt in 1..=2 {
            self.secure_logger.log_event(
                "deploy_attempt",
                json!({
                    "attempt": attempt,
                    "destination": format!(
                        "{}@{}:{}",
                        self.remote.username, self.remote.server_address, self.remote.remote_path
                    ),
                    "source": &local_path_string
                }),
            );

            let result = timeout(
                Duration::from_secs(30),
                spawn_blocking({
                    let remote = self.remote.clone();
                    let secure_logger = self.secure_logger.clone();
                    let local_path_string = local_path_string.clone();
                    move || {
                        Self::try_upload_once(&remote, &local_path_string, &secure_logger)
                    }
                }),
            )
                .await;

            match result {
                Ok(Ok(Ok(()))) => {
                    self.secure_logger.log_event(
                        "deploy_success",
                        json!({
                "attempt": attempt,
                "destination": format!(
                    "{}@{}:{}",
                    self.remote.username, self.remote.server_address, self.remote.remote_path
                )
            }),
                    );
                    return Ok(());
                }
                Ok(Ok(Err(e))) => {
                    self.secure_logger.log_error(
                        "deploy_attempt_failed",
                        &format!("Fallo lógico en intento {}: {}", attempt, e),
                    );
                }
                Ok(Err(join_err)) => {
                    self.secure_logger.log_error(
                        "deploy_attempt_panic",
                        &format!("Pánico en spawn_blocking en intento {}: {}", attempt, join_err),
                    );
                }
                Err(_timeout_err) => {
                    self.secure_logger.log_error(
                        "deploy_attempt_timeout",
                        &format!("Timeout de 30s en intento {}", attempt),
                    );
                }
            }


        }

        Err("Fallaron ambos intentos de subida".into())
    }

    fn try_upload_once(
        remote: &RemoteConfig,
        local_path: &str,
        secure_logger: &SecureLogger,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Establecer conexión TCP con timeout
        let address = format!("{}:22", remote.server_address);
        let tcp = TcpStream::connect_timeout(
            &address
                .to_socket_addrs()?
                .next()
                .ok_or("No se pudo resolver dirección")?,
            Duration::from_secs(10),
        )?;

        // Crear sesión SSH
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;

        // Autenticación
        session.userauth_password(&remote.username, &remote.pass)?;

        if !session.authenticated() {
            return Err("Falló la autenticación SSH".into());
        }

        // Leer archivo local
        let mut local_file = std::fs::File::open(local_path)?;
        let metadata = local_file.metadata()?;
        let file_size = metadata.len();

        // Crear archivo remoto
        let mut remote_file = session.scp_send(Path::new(&remote.remote_path), 0o644, file_size, None)?;
        let mut buffer = Vec::new();
        local_file.read_to_end(&mut buffer)?;
        remote_file.write_all(&buffer)?;

        secure_logger.log_event(
            "upload_success",
            json!({
                "path": remote.remote_path,
                "message": "Archivo subido correctamente"
            }),
        );

        Ok(())
    }
}
