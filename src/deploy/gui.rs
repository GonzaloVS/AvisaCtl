use crate::app::AvisaCtlApp;
use crate::deploy::logic::{Platform, RemoteConfig};
use crate::deploy::preflight::run_preflight;
use crate::deploy::remote::deploy_to_remote_async;
use crate::securelog::logger::SecureLogger;
use crate::securelog::read_secure_log_formatted;
use chrono::Local;
use eframe::egui::{self, Context, Margin, RichText};
use native_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn deploy_tab(app: &mut AvisaCtlApp, ctx: &Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Deploy Canary");
        ui.add_space(8.0);

        // if app.log_validated == false {
        //     app.log_validated = true;
        //
        //     match crate::securelog::validate_secure_log_integrity() {
        //         Ok(_) => {
        //             app.logs.lock().unwrap().push("Secure.log íntegro al iniciar la app.".to_string());
        //             app.log_valid = Some(true);
        //         }
        //         Err(e) => {
        //             app.logs.lock().unwrap().push(format!("Secure.log corrupto: {}", e));
        //             app.log_valid = Some(false);
        //         }
        //     }
        // }

        if !app.log_validated {
            app.log_validated = true;

            // 1. Si no hay ruta guardada, pedimos una carpeta
            if app.config.secure_log_path.is_none() {
                if let Some(folder) = FileDialog::new()
                    .set_title("Selecciona la carpeta donde guardar secure.log")
                    .show_open_single_dir()
                    .ok()
                    .flatten()
                {
                    let path_str = folder.display().to_string();
                    let mut new_config = app.config.clone();
                    new_config.secure_log_path = Some(path_str.clone());

                    if let Err(e) = crate::config::save_config(&new_config) {
                        app.logs
                            .lock()
                            .unwrap()
                            .push(format!("Error guardando configuración: {e}"));
                        app.log_valid = Some(false);
                        return;
                    }

                    app.config = new_config;
                    app.logs
                        .lock()
                        .unwrap()
                        .push(format!("Ruta de secure.log guardada: {}", path_str));
                } else {
                    app.logs
                        .lock()
                        .unwrap()
                        .push("No se seleccionó una carpeta para el secure.log.".to_string());
                    app.log_valid = Some(false);
                    return;
                }
            }

            // 2. Ya tenemos la ruta, la usamos
            let log_folder = app.config.secure_log_path.as_ref().unwrap();

            // 3. Intentamos inicializar el archivo si no existe
            if let Err(e) = crate::securelog::ensure_secure_log_initialized_at(log_folder) {
                app.logs
                    .lock()
                    .unwrap()
                    .push(format!("Error al inicializar secure.log: {e}"));
                app.log_valid = Some(false);
                return;
            }

            // 4. Validamos el secure.log existente
            match crate::securelog::validate_secure_log_integrity_at(log_folder) {
                Ok(_) => {
                    app.logs
                        .lock()
                        .unwrap()
                        .push("secure.log verificado: íntegro.".to_string());
                    app.log_valid = Some(true);
                }
                Err(e) => {
                    app.logs
                        .lock()
                        .unwrap()
                        .push(format!("secure.log corrupto: {e}"));
                    app.log_valid = Some(false);
                }
            }
        }

        if let Some(valid) = app.log_valid {
            let color = if valid {
                egui::Color32::from_rgb(0, 180, 0)
            } else {
                egui::Color32::from_rgb(200, 40, 40)
            };

            let message = if valid {
                "secure.log verificado: íntegro"
            } else {
                "ALERTA: secure.log alterado o corrupto"
            };

            egui::Frame::default()
                .fill(color)
                .inner_margin(Margin::symmetric(10.0 as i8, 6.0 as i8))
                .show(ui, |ui| {
                    ui.label(RichText::new(message).strong().color(egui::Color32::WHITE));
                });
        }

        ui.group(|ui| {
            ui.label(RichText::new("Proyecto").strong());
            ui.horizontal(|ui| {
                if ui.button("Seleccionar carpeta").clicked() {
                    if let Some(path) = FileDialog::new().show_open_single_dir().ok().flatten() {
                        app.project_path = Some(path.display().to_string());
                    }
                }

                if let Some(path) = &app.project_path {
                    ui.label(format!("Proyecto: {}", path));
                } else {
                    ui.label("No seleccionado.");
                }
            });
        });

        ui.add_space(10.0);

        ui.group(|ui| {
            ui.label(RichText::new("Configuración").strong());

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

        ui.add_space(10.0);

        if !app.is_deploying {
            if ui.button("▶ Iniciar Deploy").clicked() {
                app.logs.lock().unwrap().clear();
                app.is_deploying = true;
                app.cancel_deploy = false;

                let log_path = {
                    let mut p = PathBuf::from(app.config.secure_log_path.as_ref().unwrap());
                    p.push("secure.log");
                    p
                };
                let validation = crate::securelog::validate_secure_log_integrity_path(&log_path);
                if let Err(e) = validation {
                    app.logs.lock().unwrap().push(format!(
                        "No se puede continuar: secure.log inválido.\n{}",
                        e
                    ));
                    return;
                }

                let secure_logger = Arc::new(SecureLogger::new());

                if let Some(path) = &app.project_path {
                    if app.platform != Platform::Linux {
                        app.logs
                            .lock()
                            .unwrap()
                            .push("Solo se permite compilar para Linux.".to_string());
                        return;
                    }

                    app.logs
                        .lock()
                        .unwrap()
                        .push(format!("Plataforma: {:?}", app.platform));
                    app.logs
                        .lock()
                        .unwrap()
                        .push("Destino: Servidor".to_string());
                    app.logs.lock().unwrap().push(format!(
                        "Timestamp: {}",
                        Local::now().format("%Y%m%d-%H:%M:%S")
                    ));

                    let path = path.clone();
                    let logs_arc = Arc::clone(&app.logs);
                    let platform = app.platform.clone();
                    let secure_logger = Arc::clone(&secure_logger);
                    let config = app.config.clone();
                    let cancel_flag = Arc::new(Mutex::new(app.cancel_deploy));
                    let cancel_flag_clone = Arc::clone(&cancel_flag);
                    let remote_cfg = RemoteConfig {
                        server_address: app.server_address.clone(),
                        username: app.remote_user.clone(),
                        pass: app.remote_pass.clone(),
                        remote_path: app.remote_path.clone(),
                        secure_log_path: app.secure_log_path.clone(),
                    };

                    tokio::spawn(async move {
                        logs_arc
                            .lock()
                            .unwrap()
                            .push("Iniciando validaciones...".to_string());

                        let mut temp_logs = Vec::new();

                        if run_preflight(&path, &mut temp_logs, &platform, &secure_logger).await {
                            for line in temp_logs.drain(..) {
                                logs_arc.lock().unwrap().push(line);
                            }

                            let callback_logs = Arc::clone(&logs_arc);
                            let app_logs = Arc::clone(&logs_arc);

                            deploy_to_remote_async(
                                path,
                                platform,
                                remote_cfg,
                                config,
                                move |success| {
                                    let mut logs = callback_logs.lock().unwrap();
                                    if success {
                                        logs.push(
                                            "Deploy remoto completado con éxito.".to_string(),
                                        );
                                    } else {
                                        logs.push("Error durante el deploy remoto.".to_string());
                                    }

                                    let mut logs = app_logs.lock().unwrap();
                                    logs.push("Estado: deploy finalizado.".into());
                                },
                                cancel_flag_clone,
                                secure_logger,
                            );
                        } else {
                            logs_arc
                                .lock()
                                .unwrap()
                                .push("Se detuvo el deploy por error previo.".to_string());
                        }
                    });
                } else {
                    app.logs
                        .lock()
                        .unwrap()
                        .push("No se seleccionó ningún proyecto.".to_string());
                }
            }
        } else if ui.button("Cancelar Deploy").clicked() {
            app.cancel_deploy = true;
            app.logs
                .lock()
                .unwrap()
                .push("Cancelación solicitada.".into());
        }

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
                    for line in &*app.logs.lock().unwrap() {
                        ui.label(line);
                    }
                });
        });

        ui.add_space(12.0);
        ui.separator();
        //ui.label(RichText::new("Secure Log (hash encadenado)").strong());

        ui.horizontal(|ui| {
            ui.label(RichText::new("Secure Log (hash encadenado)").strong());

            if ui.button("Validar integridad").clicked() {
                let log_path = {
                    let mut p = PathBuf::from(app.config.secure_log_path.as_ref().unwrap());
                    p.push("secure.log");
                    p
                };
                match crate::securelog::validate_secure_log_integrity_path(&log_path) {
                    Ok(_) => {
                        app.logs
                            .lock()
                            .unwrap()
                            .push("El secure.log es íntegro.".to_string());
                    }
                    Err(e) => {
                        app.logs
                            .lock()
                            .unwrap()
                            .push(format!("Integridad rota: {}", e));
                    }
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
                        ui.label("⚠ secure.log vacío o no inicializado.");
                    } else {
                        for line in lines {
                            ui.label(line);
                        }
                    }
                });
        });

        // if ui.button("Validar integridad del log").clicked() {
        //     match crate::securelog::validate_secure_log_integrity() {
        //         Ok(_) => {
        //             app.logs.lock().unwrap().push("El secure.log es íntegro.".to_string());
        //         }
        //         Err(e) => {
        //             app.logs.lock().unwrap().push(format!("Integridad rota: {}", e));
        //         }
        //     }
        // }
    });
}
