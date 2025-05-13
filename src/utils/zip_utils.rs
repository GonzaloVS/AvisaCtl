use std::fs::{File, remove_file};
use std::io::{BufReader, Read, Write};
use std::path::Path;
use zip::write::FileOptions;

use ssh2::Session;

/// Crea un ZIP con los archivos indicados.
/// Los archivos se agregan con su nombre base (sin rutas).
pub fn zip_files(files: &[&str], output_zip_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output_zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for path_str in files {
        let path = Path::new(path_str);
        let name = path.file_name().ok_or("Archivo sin nombre válido")?.to_string_lossy();
        let mut f = BufReader::new(File::open(path)?);
        zip.start_file(name, options)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    zip.finish()?;
    Ok(())
}

/// Ejecuta `unzip` remotamente para descomprimir el archivo ZIP en el servidor.
pub fn unzip_remote(
    ssh: &Session,
    zip_path: &str,
    target_dir: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cmd = format!("unzip -o {} -d {}", zip_path, target_dir);
    let mut channel = ssh.channel_session()?;
    channel.exec(&cmd)?;

    let mut stdout = String::new();
    channel.read_to_string(&mut stdout).ok();

    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).ok();

    channel.send_eof()?;
    channel.wait_eof()?;
    channel.wait_close()?;
    let exit_status = channel.exit_status()?;

    if exit_status != 0 {
        return Err(format!(
            "Error al descomprimir remotamente:\nstdout: {}\nstderr: {}",
            stdout.trim(),
            stderr.trim()
        )
            .into());
    }

    Ok(())
}

/// Borra el archivo ZIP local después de subirlo, si se desea.
pub fn cleanup_local_file(path: &str) -> std::io::Result<()> {
    if Path::new(path).exists() {
        remove_file(path)?;
    }
    Ok(())
}
