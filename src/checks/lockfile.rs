use std::path::{Path, PathBuf};
use std::process::Command;

pub enum LockfileCheck {
    Missing,
    Outdated,
    DirtyGitState,
    Ok,
}

pub struct LockfileCheckResult {
    pub status: LockfileCheck,
    pub lockfile_path: PathBuf,
}

/// Verifica si Cargo.lock existe, está actualizado y no ha sido modificado manualmente
pub fn check_lockfile(project_path: &Path) -> Result<LockfileCheckResult, String> {
    let lockfile_path = project_path.join("Cargo.lock");

    if !lockfile_path.exists() {
        return Ok(LockfileCheckResult {
            status: LockfileCheck::Missing,
            lockfile_path,
        });
    }

    // Comprobar si `cargo check` detecta desincronización
    let output = Command::new("cargo")
        .args(["check"])
        .current_dir(project_path)
        .output()
        .map_err(|e| format!("No se pudo ejecutar cargo check: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("Cargo.lock needs to be updated") {
        return Ok(LockfileCheckResult {
            status: LockfileCheck::Outdated,
            lockfile_path,
        });
    }

    // Comprobar si el archivo fue modificado manualmente (solo si hay git)
    let git_status = Command::new("git")
        .args(["status", "--porcelain", "Cargo.lock"])
        .current_dir(project_path)
        .output();

    if let Ok(git_output) = git_status {
        if !git_output.stdout.is_empty() {
            return Ok(LockfileCheckResult {
                status: LockfileCheck::DirtyGitState,
                lockfile_path,
            });
        }
    }

    Ok(LockfileCheckResult {
        status: LockfileCheck::Ok,
        lockfile_path,
    })
}
