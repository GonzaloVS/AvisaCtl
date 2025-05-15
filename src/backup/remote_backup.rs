use ssh2::Session;
use std::error::Error;
use std::io::Read;
use std::path::Path;

use crate::securelog::logger::SecureLogger;

/// Realiza un backup remoto: empaqueta binario, firma GPG y TSA previos en un ZIP
pub fn create_remote_backup(
    ssh: &Session,
    bin_path: &str,
    tsr_path: &str,
    secure_logger: &SecureLogger,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    secure_logger.log_event(
        "remote_backup_start",
        serde_json::json!({
            "bin_path": bin_path,
            "tsr_path": tsr_path
        }),
    );

    let sftp = ssh.sftp()?;

    // Leer contenido del archivo .tsr para extraer la fecha del sello TSA
    let mut file = match sftp.open(Path::new(tsr_path)) {
        Ok(f) => f,
        Err(err) => {
            secure_logger.log_event(
                "remote_backup_skipped",
                serde_json::json!({
                    "reason": ".tsr no existe, se omite backup",
                    "path": tsr_path,
                    "error": format!("{}", err),
                }),
            );
            return Ok(());
        }
    };

    let mut tsr_data = Vec::new();
    file.read_to_end(&mut tsr_data)?;
    let timestamp = extract_tsa_date(&tsr_data).unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string());

    // Crear carpeta de backup
    let parent_dir = Path::new(bin_path).parent().ok_or("No se pudo obtener directorio padre")?;
    let backup_dir = parent_dir.join(format!("backup_{}", timestamp));
    let backup_dir_str = backup_dir.to_string_lossy().replace('\\', "/");

    secure_logger.log_event("mkdir_attempt", serde_json::json!({ "dir": backup_dir_str }));
    match sftp.stat(Path::new(&backup_dir_str)) {
        Ok(_) => {
            secure_logger.log_event("mkdir_skipped_exists", serde_json::json!({ "dir": backup_dir_str }));
        }
        Err(_) => {
            sftp.mkdir(Path::new(&backup_dir_str), 0o755)?;
            secure_logger.log_event("mkdir_ok", serde_json::json!({ "dir": backup_dir_str }));
        }
    }

    // Archivos a incluir
    let bin_filename = Path::new(bin_path).file_name().unwrap().to_string_lossy();
    let asc_path = format!("{}/{}.sha256.asc", parent_dir.to_string_lossy(), bin_filename);
    let tsr_path = tsr_path.to_string();
    let zip_name = format!("backup_{}.zip", timestamp);
    let zip_path = format!("{}/{}", backup_dir_str, zip_name);

    // Crear ZIP remoto y borrar archivos originales
    let cmd = format!(
        "zip -j '{}' '{}' '{}' '{}'; rm '{}' '{}' '{}'",
        zip_path,
        bin_path,
        asc_path,
        tsr_path,
        bin_path,
        asc_path,
        tsr_path
    );

    let mut channel = ssh.channel_session()?;
    channel.exec(&cmd)?;
    let mut stdout = String::new();
    channel.read_to_string(&mut stdout).ok();
    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).ok();
    channel.send_eof()?;
    channel.wait_eof()?;
    channel.wait_close()?;
    let status = channel.exit_status()?;

    if status != 0 {
        secure_logger.log_error(
            "remote_zip_failed",
            &format!("zip remoto falló:\nstdout: {}\nstderr: {}", stdout.trim(), stderr.trim()),
        );
        return Err("Fallo al crear zip remoto".into());
    }

    secure_logger.log_event(
        "remote_backup_complete",
        serde_json::json!({ "zip": zip_path, "stdout": stdout.trim() }),
    );

    Ok(())
}

/// Extrae la fecha de emisión desde la firma TSA (.tsr) si es posible
fn extract_tsa_date(tsr_data: &[u8]) -> Option<String> {
    use chrono::{DateTime, Utc};
    use openssl::asn1::Asn1Time;
    use openssl::pkcs7::Pkcs7;
    use openssl::cms::CmsContentInfo;

    // Fallback: intentar parsear con openssl
    if let Ok(tsr) = openssl::ts::TsResp::from_der(tsr_data) {
        if let Some(time) = tsr.token().and_then(|t| t.gen_time().ok()) {
            let dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&time.to_string()).ok()?.with_timezone(&Utc);
            return Some(dt.format("%Y-%m-%dT%H-%M-%SZ").to_string());
        }
    }
    None
}
