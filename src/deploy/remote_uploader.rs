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
use crate::securelog::sign;
use crate::securelog::logger::SecureLogger;
use crate::utils::tsr_utils::extract_tsa_date;


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

        // Generar SHA-256
        let bin_name = Path::new(local_path).file_name().unwrap().to_string_lossy();
        let sha_path = format!("{}.sha256", local_path);
        let mut bin = fs::File::open(local_path)?;
        let mut hasher = Sha256::new();
        let mut buf = Vec::new();
        bin.read_to_end(&mut buf)?;
        hasher.update(&buf);
        fs::write(&sha_path, format!("{:x}  {}", hasher.finalize(), bin_name))?;

        // Firmar con GPG
        let asc_path = format!("{}.sha256.asc", local_path);
        sign::ensure_gpg_available(Some(secure_logger))?;
        sign::sign_sha256_file(&sha_path, Some(secure_logger))?;

        // Firma TSA
        let tsq_path = format!("{}.tsq", local_path);
        let tsr_path = format!("{}.tsr", local_path);
        Command::new("openssl")
            .args(["ts", "-query", "-data", &sha_path, "-sha256", "-no_nonce", "-out", &tsq_path])
            .output()?;

        let curl_output = Command::new("curl")
            .args([
                "-H", "Content-Type: application/timestamp-query",
                "--data-binary", &format!("@{}", tsq_path),
                "https://freetsa.org/tsr",
                "-o", &tsr_path,
            ])
            .output()?;

        if !curl_output.status.success() {
            secure_logger.log_error(
                "curl_tsa_request_failed",
                &format!(
                    "Falló descarga TSA con curl:\nstdout: {}\nstderr: {}",
                    String::from_utf8_lossy(&curl_output.stdout),
                    String::from_utf8_lossy(&curl_output.stderr),
                ),
            );
            return Err("Fallo al obtener TSA desde freetsa.org".into());
        }

        // ✅ Validar que el .tsr recibido es válido
        let tsr_bytes = fs::read(&tsr_path)?;
        if let Err(e) = extract_tsa_date(&tsr_bytes) {
            secure_logger.log_error("tsa_tsr_invalid", &format!("TSR recibido inválido: {}", e));
            return Err("El archivo .tsr recibido es inválido o corrupto".into());
        }


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