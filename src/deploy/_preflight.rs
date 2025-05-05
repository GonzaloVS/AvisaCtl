// use serde_json::json;
// use std::path::Path;
// use tokio::process::Command;
//
// use crate::checks::bin_size::{check_binary_size, MAX_SIZE_BYTES};
// use crate::checks::lockfile::{check_lockfile, LockfileCheck};
// use crate::checks::unwraps::check_no_dangerous_patterns;
// use crate::deploy::docker::{build_with_docker, ensure_dockerfile_exists};
// use crate::deploy::logic::{extract_package_name, Platform};
// use crate::securelog::logger::SecureLogger;
//
// pub struct PreflightOptions {
//     pub check_format: bool,
//     pub check_warnings: bool,
//     pub check_tests: bool,
//     pub check_audit: bool,
//     pub check_unwraps: bool,
//     pub check_bin_size: bool,
//     pub max_bin_size: u64,
// }
//
// pub async fn run_preflight(
//     project_path: &str,
//     logs: &mut Vec<String>,
//     platform: &Platform,
//     secure_logger: &SecureLogger,
//     opts: &PreflightOptions,
// ) -> bool {
//     logs.push("Iniciando preflight...".to_string());
//
//     let mut steps = Vec::new();
//
//     if opts.check_format {
//         steps.push(("cargo fmt --check", vec!["fmt", "--", "--check"]));
//     }
//     if opts.check_warnings {
//         steps.push(("cargo clippy", vec!["clippy", "--", "-D", "warnings"]));
//     }
//     if opts.check_tests {
//         steps.push(("cargo test", vec!["test"]));
//     }
//     if opts.check_audit {
//         steps.push(("cargo audit", vec!["audit"]));
//     }
//
//     for (name, args) in steps {
//         let mut cmd = Command::new("cargo");
//         cmd.args(args).current_dir(project_path);
//
//         logs.push(format!("Ejecutando {}...", name));
//
//         match cmd.output().await {
//             Ok(output) if output.status.success() => {
//                 logs.push(format!("{} superado.", name));
//             }
//             Ok(output) => {
//                 logs.push(format!("{} falló:", name));
//                 logs.push(String::from_utf8_lossy(&output.stderr).to_string());
//                 return false;
//             }
//             Err(e) => {
//                 logs.push(format!("Error ejecutando {}: {}", name, e));
//                 return false;
//             }
//         }
//     }
//
//     if opts.check_unwraps {
//         logs.push("Comprobando uso peligroso de unwrap/expect...".to_string());
//         match check_no_dangerous_patterns(Path::new(project_path)) {
//             Ok(_) => {
//                 logs.push("Sin unwraps/expect peligrosos.".to_string());
//                 secure_logger.log_event(
//                     "preflight_unwrap_check",
//                     json!({ "status": "ok", "message": "No se detectaron unwraps peligrosos." }),
//                 );
//             }
//             Err(errors) => {
//                 logs.push("Detectado uso peligroso de unwrap/expect:".to_string());
//                 for e in &errors {
//                     logs.push(format!(" - {}", e));
//                 }
//
//                 secure_logger.log_error(
//                     "preflight_unwrap_check_failed",
//                     &format!("Se detectaron usos peligrosos:\n{}", errors.join("\n")),
//                 );
//
//                 return false;
//             }
//         }
//     }
//
//     logs.push("Verificando Cargo.lock...".to_string());
//     match check_lockfile(Path::new(project_path)) {
//         Ok(result) => match result.status {
//             LockfileCheck::Missing => {
//                 logs.push(format!(
//                     "Cargo.lock no existe en {}.",
//                     result.lockfile_path.display()
//                 ));
//                 secure_logger.log_error(
//                     "preflight_lockfile_missing",
//                     &format!(
//                         "Falta el archivo Cargo.lock en {}. Ejecuta 'cargo build' para generarlo.",
//                         result.lockfile_path.display()
//                     ),
//                 );
//                 return false;
//             }
//             LockfileCheck::Outdated => {
//                 logs.push(format!(
//                     "Cargo.lock desactualizado en {}.",
//                     result.lockfile_path.display()
//                 ));
//                 secure_logger.log_error(
//                     "preflight_lockfile_outdated",
//                     &format!(
//                         "El archivo Cargo.lock en {} está desactualizado. Ejecuta 'cargo update'.",
//                         result.lockfile_path.display()
//                     ),
//                 );
//                 return false;
//             }
//             LockfileCheck::DirtyGitState => {
//                 logs.push(format!(
//                     "Cargo.lock fue modificado manualmente o tiene cambios sin commitear: {}",
//                     result.lockfile_path.display()
//                 ));
//                 secure_logger.log_event(
//                     "preflight_lockfile_dirty",
//                     json!({
//                         "message": format!(
//                             "Cargo.lock con cambios detectados: {}",
//                             result.lockfile_path.display()
//                         )
//                     }),
//                 );
//             }
//             LockfileCheck::Ok => {
//                 logs.push(format!(
//                     "Cargo.lock verificado correctamente en {}.",
//                     result.lockfile_path.display()
//                 ));
//                 secure_logger.log_event(
//                     "preflight_lockfile_ok",
//                     json!({ "message": format!(
//                         "Cargo.lock válido y sincronizado: {}",
//                         result.lockfile_path.display()
//                     )}),
//                 );
//             }
//         },
//         Err(e) => {
//             logs.push(format!("Error verificando Cargo.lock: {}", e));
//             secure_logger.log_error("preflight_lockfile_error", &e);
//             return false;
//         }
//     }
//
//     if opts.check_bin_size {
//         logs.push("Verificando tamaño del binario...".to_string());
//         match check_binary_size(Path::new(project_path), platform, Some(opts.max_bin_size)) {
//             Ok(result) if !result.exists => {
//                 logs.push(
//                     "No se encontró binario release. ¿Ejecutaste cargo build --release?"
//                         .to_string(),
//                 );
//                 secure_logger.log_error(
//                     "preflight_binary_missing",
//                     "El binario release no fue encontrado en target/release/",
//                 );
//                 return false;
//             }
//             Ok(result) if result.too_large => {
//                 logs.push(format!(
//                     "El binario es demasiado grande: {:.2} MB (límite: {:.2} MB)",
//                     result.bin_size_bytes as f64 / 1_048_576.0,
//                     MAX_SIZE_BYTES as f64 / 1_048_576.0
//                 ));
//                 secure_logger.log_error(
//                     "preflight_binary_too_large",
//                     &format!(
//                         "Tamaño: {} bytes. Ruta: {}",
//                         result.bin_size_bytes,
//                         result.bin_path.display()
//                     ),
//                 );
//                 return false;
//             }
//             Ok(result) => {
//                 logs.push(format!(
//                     "✓ Binario correcto: {:.2} MB",
//                     result.bin_size_bytes as f64 / 1_048_576.0
//                 ));
//                 secure_logger.log_event(
//                     "preflight_binary_size_ok",
//                     json!({
//                         "path": result.bin_path,
//                         "size_bytes": result.bin_size_bytes
//                     }),
//                 );
//             }
//             Err(e) => {
//                 logs.push(format!("Error revisando el binario: {}", e));
//                 secure_logger.log_error("preflight_binary_check_failed", &e);
//                 return false;
//             }
//         }
//     }
//
//     secure_logger.log_event(
//         "preflight_dockerfile_check",
//         json!({ "message": "Validación superada. Verificando Dockerfile..." }),
//     );
//
//     if !ensure_dockerfile_exists(project_path, logs, secure_logger) {
//         return false;
//     }
//
//     logs.push("Preflight finalizado. Construyendo Docker...".to_string());
//     secure_logger.log_event(
//         "preflight_complete",
//         json!({ "message": "Validaciones completadas." }),
//     );
//
//     build_with_docker(project_path, logs, secure_logger).await
// }
//
// pub fn rename_previous_binary(
//     project_path: &str,
//     platform: &Platform,
//     secure_logger: &SecureLogger,
// ) -> Option<String> {
//     let pkg_name = extract_package_name(&Path::new(project_path).join("Cargo.toml"))?;
//     let bin_path = Path::new(project_path)
//         .join("target")
//         .join("release")
//         .join(match platform {
//             Platform::Windows => format!("{}.exe", pkg_name),
//             Platform::Linux => pkg_name.clone(),
//         });
//
//     if bin_path.exists() {
//         let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
//         let new_path = bin_path.with_file_name(format!(
//             "{} - {}{}",
//             pkg_name,
//             timestamp,
//             if *platform == Platform::Windows {
//                 ".exe"
//             } else {
//                 ""
//             }
//         ));
//
//         match std::fs::rename(&bin_path, &new_path) {
//             Ok(_) => {
//                 secure_logger.log_event(
//                     "preflight_binary_renamed",
//                     json!({
//                         "message": format!("Binario renombrado a {}", new_path.display())
//                     }),
//                 );
//                 Some(pkg_name)
//             }
//             Err(e) => {
//                 secure_logger.log_error(
//                     "preflight_binary_rename_failed",
//                     &format!("Error al renombrar binario: {}", e),
//                 );
//                 None
//             }
//         }
//     } else {
//         secure_logger.log_event(
//             "preflight_binary_none",
//             json!({
//                 "message": "No había binario previo que renombrar."
//             }),
//         );
//         Some(pkg_name)
//     }
// }
