use crate::app::AvisaCtlApp;
use crate::config::save_config;
use crate::deploy::logic::RemoteConfig;
use crate::deploy::preflight::{run_preflight, PreflightOptions};
use crate::deploy::remote::deploy_to_remote_async;
use crate::securelog::logger::SecureLogger;
use crate::securelog::read_secure_log_formatted;
use chrono::Utc;
use eframe::egui::{self, Context, Margin, RichText};
use native_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn deploy_tab(app: &mut AvisaCtlApp, ctx: &Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Deploy Canary");
        ui.add_space(8.0);

        // Validación y creación inicial de secure.log
        if !app.log_validated {
            app.log_validated = true;

            let log_path = app.config.secure_log_path.clone().unwrap_or_default();
            let log_exists = std::path::Path::new(&log_path).exists();

            if log_path.is_empty() || !log_exists {
                if let Some(folder) = FileDialog::new()
                    .set_title("Selecciona la carpeta donde guardar secure.log")
                    .show_open_single_dir()
                    .ok()
                    .flatten()
                {
                    let mut new_config = app.config.clone();
                    let path_str = folder.display().to_string();
                    new_config.secure_log_path = Some(path_str.clone());

                    if save_config(&new_config).is_ok() {
                        app.config = new_config;
                        app.logs
                            .log(format!("Ruta de secure.log guardada: {}", path_str));
                    } else {
                        app.logs.log("Error guardando configuración.");
                        app.log_valid = Some(false);
                        return;
                    }
                } else {
                    app.logs
                        .log("No se seleccionó una carpeta para el secure.log.");
                    app.log_valid = Some(false);
                    return;
                }
            }

            let result = crate::securelog::ensure_secure_log_initialized_at(
                app.config.secure_log_path.as_ref().unwrap(),
            );
            if let Err(e) = result {
                app.logs
                    .log(format!("Error al inicializar secure.log: {e}"));
                app.log_valid = Some(false);
                return;
            }

            match crate::securelog::validate_secure_log_integrity_at(
                app.config.secure_log_path.as_ref().unwrap(),
            ) {
                Ok(_) => {
                    app.logs.log("secure.log verificado: íntegro.");
                    app.log_valid = Some(true);
                }
                Err(e) => {
                    app.logs.log(format!("secure.log corrupto: {e}"));
                    app.log_valid = Some(false);
                }
            }
        }

        if let Some(valid) = app.log_valid {
            let color = if valid {
                egui::Color32::DARK_GREEN
            } else {
                egui::Color32::RED
            };
            let text = if valid {
                "secure.log verificado: íntegro"
            } else {
                "ALERTA: secure.log alterado o corrupto"
            };

            egui::Frame::default()
                .fill(color)
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).strong().color(egui::Color32::WHITE));
                });
        }

        // Selección de proyecto
        ui.group(|ui| {
            ui.label(RichText::new("Proyecto").strong());
            ui.horizontal(|ui| {
                if ui.button("Seleccionar carpeta").clicked() {
                    if let Some(path) = FileDialog::new().show_open_single_dir().ok().flatten() {
                        app.project_path = Some(path.display().to_string());
                    }
                }

                ui.label(
                    app.project_path
                        .clone()
                        .unwrap_or_else(|| "No seleccionado.".into()),
                );
            });
        });

        ui.add_space(10.0);

        // Destino y configuración
        ui.horizontal(|ui| {
            // DESTINO
            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.label(RichText::new("Destino").strong());

                    ui.horizontal(|ui| {
                        ui.label("Servidor:");
                        ui.text_edit_singleline(&mut app.server_address);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Usuario:");
                        ui.text_edit_singleline(&mut app.remote_user);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Password:");
                        use eframe::egui::TextEdit;
                        ui.add(TextEdit::singleline(&mut app.remote_pass).password(true));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Ruta remota:");
                        ui.text_edit_singleline(&mut app.remote_path);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Plataforma:");
                        ui.label("Linux");
                    });
                });
            });

            // CONFIGURACIÓN
            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.label(RichText::new("Configuración").strong());

                    ui.label("Validaciones básicas:");
                    let mut changed = false;
                    for (label, flag) in [
                        ("format check", &mut app.config.check_format),
                        ("warnings", &mut app.config.check_warnings),
                        ("test", &mut app.config.check_tests),
                        ("audit", &mut app.config.check_audit),
                    ] {
                        if ui.checkbox(flag, label).changed() {
                            changed = true;
                        }
                    }
                    if changed {
                        let _ = save_config(&app.config);
                    }

                    ui.separator();
                    ui.label("Validaciones avanzadas:");
                    if ui
                        .checkbox(&mut app.config.check_unwraps, "unwraps")
                        .changed()
                    {
                        let _ = save_config(&app.config);
                    }

                    ui.horizontal(|ui| {
                        if ui
                            .checkbox(&mut app.config.check_bin_size, "bin size")
                            .changed()
                        {
                            let _ = save_config(&app.config);
                        }

                        if app.config.check_bin_size {
                            ui.label("Máx (bytes):");
                            ui.add(
                                egui::DragValue::new(&mut app.config.max_bin_size)
                                    .range(0..=100_000_000)
                                    .clamp_existing_to_range(true)
                                    .speed(100),
                            );
                        }
                    });
                });
            });
        });

        ui.add_space(10.0);

        // BOTÓN DE DEPLOY
        if !app.is_deploying {
            if ui.button("▶ Iniciar Deploy").clicked() {
                app.logs.get().lock().unwrap().clear();
                app.is_deploying = true;
                app.cancel_deploy = false;

                let secure_logger = Arc::new(SecureLogger::new());

                if let Some(project_path) = &app.project_path {
                    let preflight_opts = PreflightOptions {
                        check_format: app.config.check_format,
                        check_warnings: app.config.check_warnings,
                        check_tests: app.config.check_tests,
                        check_audit: app.config.check_audit,
                        check_unwraps: app.config.check_unwraps,
                        check_bin_size: app.config.check_bin_size,
                        max_bin_size: app.config.max_bin_size,
                    };

                    let remote_cfg = RemoteConfig {
                        server_address: app.server_address.clone(),
                        username: app.remote_user.clone(),
                        pass: app.remote_pass.clone(),
                        remote_path: app.remote_path.clone(),
                        secure_log_path: app.config.secure_log_path.clone(),
                    };

                    let logs_arc = app.logs.clone();
                    let cancel_flag = Arc::new(Mutex::new(app.cancel_deploy));
                    let config = app.config.clone();

                    tokio::spawn({
                        let path = project_path.clone();
                        let platform = app.platform.clone();
                        let secure_logger = Arc::clone(&secure_logger);
                        let cancel_flag_clone = Arc::clone(&cancel_flag);
                        let callback_logs = logs_arc.get();

                        async move {
                            logs_arc.log("Iniciando validaciones...");

                            let mut temp_logs = Vec::new();

                            if run_preflight(
                                &path,
                                &mut temp_logs,
                                &platform,
                                &secure_logger,
                                &preflight_opts,
                            )
                            .await
                            {
                                for line in temp_logs.drain(..) {
                                    logs_arc.get().lock().unwrap().push((Utc::now(), line));
                                }

                                deploy_to_remote_async(
                                    path,
                                    platform,
                                    remote_cfg,
                                    config,
                                    move |success| {
                                        let mut logs = callback_logs.lock().unwrap();
                                        if success {
                                            logs.push((
                                                Utc::now(),
                                                "Deploy remoto completado con éxito.".into(),
                                            ));
                                        } else {
                                            logs.push((
                                                Utc::now(),
                                                "Error durante el deploy remoto.".into(),
                                            ));
                                        }
                                        logs.push((
                                            Utc::now(),
                                            "Estado: deploy finalizado.".into(),
                                        ));
                                    },
                                    cancel_flag_clone,
                                    secure_logger,
                                );
                            } else {
                                for line in temp_logs.drain(..) {
                                    logs_arc.get().lock().unwrap().push((Utc::now(), line));
                                }
                                logs_arc.get().lock().unwrap().push((
                                    Utc::now(),
                                    "Se detuvo el deploy por error previo.".into(),
                                ));
                            }
                        }
                    });
                }
            }
        } else if ui.button("Cancelar Deploy").clicked() {
            app.cancel_deploy = true;
            app.logs.log("Cancelación solicitada.");
        }

        // LOGS
        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new("Log de acciones").strong());

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_height(150.0);
            egui::ScrollArea::vertical()
                .id_salt("log_scroll_area")
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for (timestamp, line) in &*app.logs.get().lock().unwrap() {
                        let label = format!("[{}] {}", timestamp.format("%Y-%m-%d %H:%M:%S"), line);
                        ui.label(label);
                    }
                });
        });

        ui.add_space(12.0);
        ui.separator();

        // Secure log
        ui.horizontal(|ui| {
            ui.label("Ruta log:");
            ui.label(
                app.config
                    .secure_log_path
                    .as_deref()
                    .unwrap_or("No configurada"),
            );
        });

        ui.horizontal(|ui| {
            ui.label(RichText::new("Secure Log (hash encadenado)").strong());
            if ui.button("Validar integridad").clicked() {
                let log_path =
                    PathBuf::from(app.config.secure_log_path.as_ref().unwrap()).join("secure.log");
                match crate::securelog::validate_secure_log_integrity_path(&log_path) {
                    Ok(_) => app.logs.log("El secure.log es íntegro."),
                    Err(e) => app.logs.log(format!("Integridad rota: {}", e)),
                }
            }
        });

        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("secure_log_scroll_area")
                .max_height(150.0)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let lines = read_secure_log_formatted();
                    if lines.is_empty() {
                        ui.label("secure.log vacío o no inicializado.");
                    } else {
                        for line in lines {
                            ui.label(line);
                        }
                    }
                });
        });
    });
}
