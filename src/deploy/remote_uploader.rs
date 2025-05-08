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

    pub async fn upload_binary(
        &self,
        local_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let local_path_string = local_path.to_string_lossy().to_string();

        let (local_timestamp_path, timestamp_value) =
            generate_local_timestamp_file(&local_path_string, &self.secure_logger)?;

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
                    let local_timestamp_path = local_timestamp_path.clone();
                    let timestamp_value = timestamp_value.clone();
                    move || {
                        Self::try_upload_once(
                            &remote,
                            &local_path_string,
                            &local_timestamp_path,
                            &timestamp_value,
                            &secure_logger,
                        )
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
                        &format!(
                            "Pánico en spawn_blocking en intento {}: {}",
                            attempt, join_err
                        ),
                    );
                }
                Err(timeout_err) => {
                    self.secure_logger.log_error(
                        "deploy_attempt_timeout",
                        &format!("Timeout en intento {}: {:?}", attempt, timeout_err),
                    );
                }
            }
        }

        Err("Fallaron ambos intentos de subida".into())
    }

    fn find_existing_timestamp_file(
        session: &Session,
        remote_path: &str,
        logger: &SecureLogger,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let sftp = session.sftp()?;

        // let dir = sftp.opendir(Path::new(remote_path))?;
        // let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z$")?;
        // for entry in dir {
        //     let entry = entry?;
        //     if let Some(filename) = entry.filename() {
        //         if re.is_match(&filename) {
        //             let path = format!("{}/{}", remote_path, filename);
        //             logger.log_event("found_existing_timestamp", json!({ "path": path }));
        //             return Ok(path);
        //         }
        //     }
        // }

        let entries = sftp.readdir(Path::new(remote_path))?;

        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}Z$")?;

        for (path, _stat) in entries {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if re.is_match(file_name) {
                    let full_path = format!("{}/{}", remote_path, file_name);
                    logger.log_event("found_existing_timestamp", json!({ "path": full_path }));
                    return Ok(full_path);
                }
            }
        }


        logger.log_event("no_existing_timestamp_found", json!({ "dir": remote_path }));
        Err("No se encontró archivo timestamp en el servidor".into())
    }

    fn try_upload_once(
        remote: &RemoteConfig,
        local_path: &str,
        local_timestamp_path: &str,
        timestamp_filename: &str,
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

        // Hacer backup remoto antes de subir
        let remote_bin_path = format!("{}/{}", remote.remote_path, Path::new(local_path).file_name().ok_or("Nombre de binario inválido")?.to_string_lossy());
        let remote_timestamp_path = Self::find_existing_timestamp_file(&session, &remote.remote_path, secure_logger)?;

        crate::backup::remote_backup::create_remote_backup(
            &session,
            &remote_bin_path,
            &remote_timestamp_path,
            secure_logger,
        )?;

        // Inicializar SFTP
        let sftp = session.sftp()?;

        // Preparar ruta remota (con nombre del binario)
        let filename = Path::new(local_path)
            .file_name()
            .ok_or("No se pudo obtener nombre del archivo")?
            .to_string_lossy();
        let remote_file_path = format!("{}/{}", remote.remote_path, filename);

        // Leer archivo local
        let mut local_file = std::fs::File::open(local_path)?;
        let mut buffer = Vec::new();
        local_file.read_to_end(&mut buffer)?;

        // Crear archivo remoto vía SFTP
        use ssh2::OpenFlags;
        let mut remote_file = sftp.open_mode(
            Path::new(&remote_file_path),
            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE,
            0o644,
            ssh2::OpenType::File,
        )?;

        remote_file.write_all(&buffer)?;

        secure_logger.log_event(
            "upload_success",
            json!({
            "path": remote_file_path,
            "message": "Archivo subido correctamente via SFTP"
        }),
        );

        Self::upload_timestamp_file(
            &sftp,
            local_timestamp_path,
            &remote_timestamp_path,
            secure_logger,
        )?;

        secure_logger.log_event(
            "upload_success",
            json!({
            "path": remote_file_path,
            "message": "Timestamp subido correctamente via SFTP"
        }),
        );

        Ok(())
    }

    fn upload_timestamp_file(
        sftp: &ssh2::Sftp,
        local_timestamp_path: &str,
        remote_timestamp_path: &str,
        secure_logger: &SecureLogger,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use ssh2::OpenFlags;
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(local_timestamp_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut remote_file = sftp.open_mode(
            Path::new(remote_timestamp_path),
            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE,
            0o644,
            ssh2::OpenType::File,
        )?;
        remote_file.write_all(&buffer)?;

        let timestamp_str = String::from_utf8_lossy(&buffer);
        secure_logger.log_event(
            "timestamp_uploaded",
            json!({
                "remote_path": remote_timestamp_path,
                "value": timestamp_str.trim()
            }),
        );

        Ok(())
    }

}

fn generate_local_timestamp_file(
    local_binary_path: &str,
    secure_logger: &SecureLogger,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    use chrono::Utc;
    use std::fs;

    let timestamp = Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();
    let local_dir = Path::new(local_binary_path)
        .parent()
        .ok_or("No se pudo determinar el directorio del binario")?;
    let local_timestamp_path = local_dir.join(&timestamp);
    fs::write(&local_timestamp_path, &timestamp)?;

    secure_logger.log_event(
        "timestamp_generated",
        json!({
            "local_path": local_timestamp_path,
            "value": timestamp
        }),
    );

    Ok((local_timestamp_path.to_string_lossy().to_string(), timestamp))


}