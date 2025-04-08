use crate::app::AvisaCtlApp;
use crate::deploy::local::{rename_previous_binary_if_exists, run_pre_release_checks};
use crate::deploy::logic::{DeployTarget, Platform, RemoteConfig};
use crate::deploy::remote::deploy_to_remote_async;
use chrono::Local;
use eframe::egui::{self, Context, Margin, RichText};
use native_dialog::FileDialog;
use std::sync::{Arc, Mutex};
use crate::helper::logger_secure::{log_to_vec, read_secure_log_formatted, SecureLogger};

pub fn deploy_tab(app: &mut AvisaCtlApp, ctx: &Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Deploy Canary");
        ui.add_space(8.0);

        if app.log_validated == false {
            app.log_validated = true;

            match crate::helper::logger_secure::validate_secure_log_integrity() {
                Ok(_) => {
                    app.logs.lock().unwrap().push("Secure.log íntegro al iniciar la app.".to_string());
                    app.log_valid = Some(true);
                }
                Err(e) => {
                    app.logs.lock().unwrap().push(format!("Secure.log corrupto: {}", e));
                    app.log_valid = Some(false);
                }
            }
        }

        // Banner visual
        if let Some(valid) = app.log_valid {
            let color = if valid {
                egui::Color32::from_rgb(0, 180, 0) // verde
            } else {
                egui::Color32::from_rgb(200, 40, 40) // rojo
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

                //let secure_logger = Arc::new(SecureLogger::new());

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
                ui.label("Destino:");
                ui.selectable_value(&mut app.target, DeployTarget::Local, "Local");
                ui.selectable_value(&mut app.target, DeployTarget::Remote, "Servidor");
            });

            if app.target == DeployTarget::Remote {
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
            }

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

                let validation = crate::helper::logger_secure::validate_secure_log_integrity();
                if let Err(e) = validation {
                    app.logs
                        .lock()
                        .unwrap()
                        .push(format!("No se puede continuar: secure.log inválido.\n{}", e));
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
                        .push(format!("Destino: {:?}", app.target));
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
                    let target = app.target.clone();
                    let remote_cfg = RemoteConfig {
                        server_address: app.server_address.clone(),
                        username: app.remote_user.clone(),
                        pass: app.remote_pass.clone(),
                        remote_path: app.remote_path.clone(),
                    };

                    tokio::spawn(async move {
                        log_to_vec(&logs_arc, "Iniciando validaciones...");

                        // Usamos un buffer temporal para logs que luego mergeamos
                        let mut temp_logs = Vec::new();

                        if run_pre_release_checks(&path, &mut temp_logs, &platform, &secure_logger).await {
                            for line in temp_logs.drain(..) {
                                log_to_vec(&logs_arc, line);
                            }
                            if target == DeployTarget::Remote {
                                let callback_logs = Arc::clone(&logs_arc);
                                let app_logs = Arc::clone(&logs_arc);

                                deploy_to_remote_async(
                                    path,
                                    logs_arc,
                                    platform,
                                    remote_cfg,
                                    config,
                                    move |success| {
                                        let mut logs = callback_logs.lock().unwrap();
                                        if success {
                                            logs.push("Deploy remoto completado con éxito.".to_string());
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
                                let _ = rename_previous_binary_if_exists(
                                    &path,
                                    &mut logs_arc.lock().unwrap(),
                                    &platform,
                                    &secure_logger,
                                );
                                logs_arc
                                    .lock()
                                    .unwrap()
                                    .push("Deploy local completado con éxito.".to_string());
                            }
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

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &*app.logs.lock().unwrap() {
                    ui.label(line);
                }
            });

        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new("Secure Log (hash encadenado)").strong());

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in read_secure_log_formatted() {
                    ui.label(line);
                }
            });

        if ui.button("Validar integridad del log").clicked() {
            match crate::helper::logger_secure::validate_secure_log_integrity() {
                Ok(_) => {
                    app.logs.lock().unwrap().push("El secure.log es íntegro.".to_string());
                }
                Err(e) => {
                    app.logs.lock().unwrap().push(format!("Integridad rota: {}", e));
                }
            }
        }
    });
}
