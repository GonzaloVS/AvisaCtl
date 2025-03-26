use crate::app::AvisaCtlApp;
use crate::deploy::local::{rename_previous_binary_if_exists, run_pre_release_checks};
use crate::deploy::logic::{DeployTarget, Platform, RemoteConfig};
use crate::deploy::remote::deploy_to_remote;
use chrono::Local;
use eframe::egui::{self, Context, RichText};
use native_dialog::FileDialog;

pub fn deploy_tab(app: &mut AvisaCtlApp, ctx: &Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Deploy Canary");
        ui.add_space(8.0);

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

        if ui.add(egui::Button::new("Iniciar Deploy")).clicked() {
            app.logs.clear();

            if let Some(path) = &app.project_path {
                if app.platform != Platform::Linux {
                    app.logs
                        .push("Solo se permite compilar para Linux.".to_string());
                    return;
                }

                app.logs.push(format!("Plataforma: {:?}", app.platform));
                app.logs.push(format!("Destino: {:?}", app.target));
                app.logs.push(format!(
                    "Timestamp: {}",
                    Local::now().format("%Y%m%d-%H:%M:%S")
                ));

                let success = run_pre_release_checks(path, &mut app.logs, &app.platform);
                if success {
                    if app.target == DeployTarget::Remote {
                        let remote_cfg = RemoteConfig {
                            server_address: app.server_address.clone(),
                            username: app.remote_user.clone(),
                            pass: app.remote_pass.clone(),
                            remote_path: app.remote_path.clone(),
                        };

                        let uploaded = deploy_to_remote(
                            path,
                            &mut app.logs,
                            &app.platform,
                            &remote_cfg,
                            &mut app.config,
                        );

                        if uploaded {
                            app.logs
                                .push("Deploy remoto completado con éxito.".to_string());
                        } else {
                            app.logs.push("Error durante el deploy remoto.".to_string());
                        }
                    } else {
                        let _ =
                            rename_previous_binary_if_exists(path, &mut app.logs, &app.platform);
                        app.logs
                            .push("Deploy local completado con éxito.".to_string());
                    }
                } else {
                    app.logs
                        .push("Se detuvo el deploy por error previo.".to_string());
                }
            } else {
                app.logs
                    .push("No se seleccionó ningún proyecto.".to_string());
            }
        }

        ui.add_space(12.0);
        ui.separator();

        ui.label(RichText::new("Log de acciones").strong());

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &app.logs {
                    ui.label(line);
                }
            });
    });
}
