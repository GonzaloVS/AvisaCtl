use crate::deploy::logic::{extract_package_name, Platform};
use crate::securelog::logger::SecureLogger;
use serde_json::json;
use std::path::{Path};

pub fn rename_previous_binary(
    project_path: &str,
    platform: &Platform,
    secure_logger: &SecureLogger,
) -> Option<String> {
    let pkg_name = extract_package_name(&Path::new(project_path).join("Cargo.toml"))?;
    let bin_path = Path::new(project_path)
        .join("target")
        .join("release")
        .join(match platform {
            Platform::Windows => format!("{}.exe", pkg_name),
            Platform::Linux => pkg_name.clone(),
        });

    if bin_path.exists() {
        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let new_path = bin_path.with_file_name(format!(
            "{} - {}{}",
            pkg_name,
            timestamp,
            if *platform == Platform::Windows {
                ".exe"
            } else {
                ""
            }
        ));

        match std::fs::rename(&bin_path, &new_path) {
            Ok(_) => {
                secure_logger.log_event("preflight_binary_renamed", json!({
                    "message": format!("Binario renombrado a {}", new_path.display())
                }));
                Some(pkg_name)
            }
            Err(e) => {
                secure_logger.log_error("preflight_binary_rename_failed", &e.to_string());
                None
            }
        }
    } else {
        secure_logger.log_event("preflight_binary_none", json!({
            "message": "No había binario previo que renombrar."
        }));
        Some(pkg_name)
    }
}
