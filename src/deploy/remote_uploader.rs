use serde_json::json;
use ssh2::Session;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;
use tokio::time::timeout;

use crate::deploy::remote::config::RemoteConfig;
use crate::securelog::logger::SecureLogger;
use crate::utils::zip_utils::{zip_files, unzip_remote, cleanup_local_file};

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
                        Self::try_upload_once(
                            &remote,
                            &local_path_string,
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
                        &format!("Pánico en spawn_blocking en intento {}: {}", attempt, join_err),
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
        secure_logger: &SecureLogger,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::process::Command;
        use std::fs;
        use std::io::{Read, Write};
        use sha2::{Sha256, Digest};
        use crate::utils::zip_utils::{zip_files, unzip_remote, cleanup_local_file};

        let tcp = TcpStream::connect_timeout(
            &format!("{}:22", remote.server_address).to_socket_addrs()?.next().ok_or("No se pudo resolver dirección")?,
            Duration::from_secs(10),
        )?;
        let mut session = Session::new()?;
        session.set_tcp_stream(tcp);
        session.handshake()?;
        session.userauth_password(&remote.username, &remote.pass)?;
        if !session.authenticated() {
            return Err("Falló la autenticación SSH".into());
        }

        // Backup remoto si hay versión anterior
        let remote_bin_path = format!("{}/{}", remote.remote_path, Path::new(local_path).file_name().unwrap().to_string_lossy());
        if let Ok(prev_tsr) = Self::find_existing_timestamp_file(&session, &remote.remote_path, secure_logger) {
            crate::backup::remote_backup::create_remote_backup(&session, &remote_bin_path, &prev_tsr, secure_logger)?;
        }

        // Generar SHA-256
        let bin_name = Path::new(local_path).file_name().unwrap().to_string_lossy();
        let sha_path = format!("{}.sha256", local_path);
        let mut bin = fs::File::open(local_path)?;
        let mut hasher = Sha256::new();
        let mut buf = Vec::new();
        bin.read_to_end(&mut buf)?;
        hasher.update(&buf);
        fs::write(&sha_path, format!("{:x}", hasher.finalize()))?;

        // Firmar con GPG
        let asc_path = format!("{}.sha256.asc", local_path);
        Command::new("gpg")
            .args(["--armor", "--output", &asc_path, "--sign", &sha_path])
            .output()?;

        // Firma TSA
        let tsq_path = format!("{}.tsq", local_path);
        let tsr_path = format!("{}.tsr", local_path);
        Command::new("openssl")
            .args(["ts", "-query", "-data", &sha_path, "-sha256", "-no_nonce", "-out", &tsq_path])
            .output()?;
        Command::new("curl")
            .args([
                "-H", "Content-Type: application/timestamp-query",
                "--data-binary", &format!("@{}", tsq_path),
                "https://freetsa.org/tsr",
                "-o", &tsr_path,
            ])
            .output()?;

        // Crear ZIP
        let zip_path = format!("{}.zip", local_path);
        zip_files(&[local_path, &asc_path, &tsr_path], &zip_path)?;

        // Subir ZIP
        let sftp = session.sftp()?;
        let remote_zip = format!("{}/{}", remote.remote_path, Path::new(&zip_path).file_name().unwrap().to_string_lossy());
        let mut remote_file = sftp.create(Path::new(&remote_zip))?;
        let zip_bytes = fs::read(&zip_path)?;
        remote_file.write_all(&zip_bytes)?;

        // Descomprimir remotamente
        unzip_remote(&session, &remote_zip, &remote.remote_path)?;

        // Limpiar
        cleanup_local_file(&sha_path).ok();
        cleanup_local_file(&asc_path).ok();
        cleanup_local_file(&tsq_path).ok();
        cleanup_local_file(&tsr_path).ok();
        cleanup_local_file(&zip_path).ok();

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