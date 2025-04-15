use std::fs;
use std::path::{Path, PathBuf};

use crate::deploy::logic::{extract_package_name, Platform};

/// Tamaño máximo permitido del binario en bytes (por defecto 15 MB)
pub const MAX_SIZE_BYTES: u64 = 15 * 1024 * 1024;

pub struct BinSizeCheckResult {
    pub bin_path: PathBuf,
    pub bin_size_bytes: u64,
    pub too_large: bool,
    pub exists: bool,
}

/// Devuelve la ruta del binario en `target/release` según plataforma
pub fn get_binary_path(project_path: &Path, platform: &Platform) -> Option<PathBuf> {
    let pkg_name = extract_package_name(&project_path.join("Cargo.toml"))?;
    let bin_name = match platform {
        Platform::Windows => format!("{}.exe", pkg_name),
        Platform::Linux => pkg_name,
    };

    Some(project_path.join("target").join("release").join(bin_name))
}

/// Ejecuta la verificación de tamaño de binario
pub fn check_binary_size(
    project_path: &Path,
    platform: &Platform,
    max_size: Option<u64>,
) -> Result<BinSizeCheckResult, String> {
    let max_size = max_size.unwrap_or(MAX_SIZE_BYTES);
    let bin_path = get_binary_path(project_path, platform)
        .ok_or("No se pudo determinar el nombre del paquete")?;

    if !bin_path.exists() {
        return Ok(BinSizeCheckResult {
            bin_path,
            bin_size_bytes: 0,
            too_large: false,
            exists: false,
        });
    }

    let metadata = fs::metadata(&bin_path).map_err(|e| e.to_string())?;
    let size = metadata.len();

    Ok(BinSizeCheckResult {
        bin_path,
        bin_size_bytes: size,
        too_large: size > max_size,
        exists: true,
    })
}
