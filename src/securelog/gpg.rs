
/// Verifica si hay al menos una clave GPG disponible
pub fn has_gpg_key() -> bool;

/// Genera una nueva clave GPG tipo RSA 4096 sin passphrase
pub fn generate_gpg_key(name: &str, email: &str) -> Result<(), Box<dyn Error>>;

/// Firma un archivo .sha256 → genera .sha256.asc
pub fn sign_sha256_file(input_path: &str) -> Result<(), Box<dyn Error>>;

/// Exporta clave pública
pub fn export_public_key(email: &str, output_path: &str) -> Result<(), Box<dyn Error>>;
