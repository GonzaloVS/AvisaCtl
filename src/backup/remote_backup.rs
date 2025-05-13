use ssh2::{Session};
use std::error::Error;
use std::io::Read;
use std::path::Path;

use crate::securelog::logger::SecureLogger;

/// Realiza un backup remoto del binario y del archivo de timestamp antes de la subida.
/// Crea una carpeta y mueve ahí los archivos.
pub fn create_remote_backup(
    ssh: &Session,
    bin_path: &str,
    timestamp_path: &str,
    secure_logger: &SecureLogger,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    secure_logger.log_event(
        "remote_backup_start",
        serde_json::json!({
            "bin_path": bin_path,
            "timestamp_path": timestamp_path
        }),
    );

    let sftp = ssh.sftp()?;

    // Leer contenido del archivo timestamp
    let mut file = match sftp.open(Path::new(timestamp_path)) {
        Ok(f) => f,
        Err(err) => {
            secure_logger.log_event(
                "remote_backup_skipped",
                serde_json::json!({
                    "reason": "timestamp no existe, se omite backup",
                    "path": timestamp_path,
                    "error": format!("{}", err),
                }),
            );
            return Ok(());
        }
    };

    let mut timestamp = String::new();
    file.read_to_string(&mut timestamp)?;
    let timestamp = timestamp.trim();
    if timestamp.is_empty() {
        secure_logger.log_error("remote_backup_fail", "El archivo timestamp está vacío");
        return Err("timestamp remoto vacío".into());
    }

    // Crear carpeta de backup dentro del mismo directorio del binario
    let parent_dir = Path::new(bin_path)
        .parent()
        .ok_or("No se pudo obtener directorio padre del binario")?;
    let backup_dir = parent_dir.join(format!("backup_{}", timestamp));
    let backup_dir_str = backup_dir.to_string_lossy().replace('\\', "/");

    // 🔍 DEBUG: Mostrar por terminal la ruta que se intenta crear
    println!("Intentando crear carpeta de backup en: {}", backup_dir_str);
    secure_logger.log_event(
        "mkdir_attempt",
        serde_json::json!({ "dir": backup_dir_str }),
    );

    // Verificar si la carpeta ya existe antes de intentar crearla
    match sftp.stat(Path::new(&backup_dir_str)) {
        Ok(_) => {
            // Ya existe
            println!("La carpeta ya existe, no se crea.");
            secure_logger.log_event(
                "mkdir_skipped_exists",
                serde_json::json!({ "dir": backup_dir_str }),
            );
        }
        Err(e) => {
            // No existe, intentar crearla
            println!("La carpeta no existe, se intenta crear.");
            match sftp.mkdir(Path::new(&backup_dir_str), 0o755) {
                Ok(_) => {
                    println!("Carpeta creada con éxito.");
                    secure_logger.log_event(
                        "mkdir_ok",
                        serde_json::json!({ "dir": backup_dir_str }),
                    );
                }
                Err(e) => {
                    println!("Error al crear carpeta: {}", e);
                    secure_logger.log_error(
                        "mkdir_failed",
                        &format!("No se pudo crear el directorio {} → {}", backup_dir_str, e),
                    );
                    return Err(e.into());
                }
            }
        }
    }

    // Mover el binario
    let bin_filename = Path::new(bin_path)
        .file_name()
        .ok_or("No se pudo obtener nombre del binario")?;
    let bin_backup_path = backup_dir.join(bin_filename);
    let bin_backup_path_str = bin_backup_path.to_string_lossy().replace('\\', "/");

    sftp.rename(Path::new(bin_path), Path::new(&bin_backup_path_str), None)?;
    secure_logger.log_event(
        "remote_backup_moved_bin",
        serde_json::json!({ "to": bin_backup_path_str }),
    );

    // Mover el timestamp
    let ts_filename = Path::new(timestamp_path)
        .file_name()
        .ok_or("No se pudo obtener nombre del timestamp")?;
    let ts_backup_path = backup_dir.join(ts_filename);
    let ts_backup_path_str = ts_backup_path.to_string_lossy().replace('\\', "/");

    sftp.rename(Path::new(timestamp_path), Path::new(&ts_backup_path_str), None)?;
    secure_logger.log_event(
        "remote_backup_moved_timestamp",
        serde_json::json!({ "to": ts_backup_path_str }),
    );

    secure_logger.log_event(
        "remote_backup_complete",
        serde_json::json!({ "backup_dir": backup_dir_str }),
    );

    Ok(())
}

fn run_command(
    ssh: &Session,
    command: &str,
    secure_logger: &SecureLogger,
    label: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let mut channel = ssh.channel_session()?;

    if let Err(e) = channel.exec(command) {
        secure_logger.log_error(&format!("{}_exec_error", label), &format!("Falló exec: {} -> {}", label, e));
        return Err(e.into());
    }

    // Leer stdout y stderr
    let mut stdout = String::new();
    channel.read_to_string(&mut stdout).ok();

    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).ok();

    // Esperar correctamente cierre del canal
    channel.send_eof()?;
    channel.wait_eof()?;
    channel.wait_close()?;
    let exit_code = channel.exit_status()?;

    if exit_code != 0 {
        secure_logger.log_error(
            &format!("{}_exit_nonzero", label),
            &format!(
                "{} → código {}\nstdout: {}\nstderr: {}",
                command, exit_code, stdout.trim(), stderr.trim()
            ),
        );
        return Err(format!("El comando '{}' falló con código {}", command, exit_code).into());
    }

    secure_logger.log_event(
        &format!("{}_ok", label),
        serde_json::json!({ "cmd": command, "stdout": stdout.trim() }),
    );

    Ok(stdout)
}

