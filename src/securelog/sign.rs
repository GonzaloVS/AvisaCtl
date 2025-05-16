use sequoia_openpgp as openpgp;
use openpgp::{
    cert::prelude::*,
    packet::prelude::*,
    parse::Parse,
    policy::StandardPolicy,
    serialize::stream::{Armorer, Signer, Message},
    types::HashAlgorithm,
};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::error::Error;
use std::process::Command;
use crate::securelog::logger::SecureLogger;

pub fn has_gpg_key() -> bool {
    Command::new("gpg")
        .arg("--list-keys")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn generate_gpg_key(name: &str, email: &str) -> Result<(), Box<dyn Error>> {
    let batch = format!(
        "Key-Type: default\nKey-Length: 2048\nName-Real: {}\nName-Email: {}\nExpire-Date: 0\n%commit\n",
        name, email
    );
    std::fs::write("gpg_batch.txt", &batch)?;
    Command::new("gpg")
        .args(["--batch", "--generate-key", "gpg_batch.txt"])
        .status()?;
    Ok(())
}

pub fn sign_sha256_file(path: &str, logger: Option<&SecureLogger>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let asc_path = format!("{}.asc", path);

    // Generar un par de claves temporal en memoria (para pruebas o firma efímera)
    let (cert, keypair) = CertBuilder::general_purpose(None, Some("avisactl@localhost"))?.generate()?;
    let policy = &StandardPolicy::new();
    let signer_key = keypair
        .keys()
        .secret()
        .with_policy(policy, None)
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .ok_or("No se pudo obtener clave de firma")?;

    // Abrir archivo a firmar
    let input = File::open(path)?;
    let reader = BufReader::new(input);

    let output = File::create(&asc_path)?;
    let mut writer = BufWriter::new(output);

    // Generar firma ASCII-armored
    let message = Armorer::new(&mut writer)?.build()?;
    let mut signer = Signer::new(
        message,
        vec![SignerBuilder::new(signer_key.key(), HashAlgorithm::SHA2_256)],
    )?;

    std::io::copy(&mut reader.take(10_000_000), &mut signer)?; // Limite de seguridad
    signer.finalize()?;

    if let Some(logger) = logger {
        logger.log_event("pgp_sign_success", json!({ "output": asc_path }));
    }

    Ok(())
}

pub fn export_public_key(email: &str, output_path: &str) -> Result<(), Box<dyn Error>> {
    Command::new("gpg")
        .args(["--armor", "--export", email])
        .output()
        .map(|out| std::fs::write(output_path, out.stdout))??;
    Ok(())
}


pub fn ensure_gpg_available(logger: Option<&SecureLogger>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let output = Command::new("gpg")
        .arg("--version")
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            if let Some(logger) = logger {
                logger.log_error(
                    "gpg_not_available",
                    &format!(
                        "`gpg` no disponible o falló:\nstdout: {}\nstderr: {}",
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr),
                    ),
                );
            }
            Err("GPG no está instalado o no se puede ejecutar".into())
        }
        Err(e) => {
            if let Some(logger) = logger {
                logger.log_error("gpg_not_found", &format!("Error ejecutando gpg: {}", e));
            }
            Err("No se pudo ejecutar `gpg`".into())
        }
    }
}