use crate::app::AvisaCtlApp;
use crate::config::save_config;
use crate::deploy::logic::{
    rename_previous_binary_if_exists, run_pre_release_checks, DeployTarget, Platform,
};
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

                if app.target == DeployTarget::Remote {
                    app.config.last_server_address = app.server_address.clone();
                    save_config(&app.config);
                }

                app.logs.push(format!("Plataforma: {:?}", app.platform));
                app.logs.push(format!("Destino: {:?}", app.target));
                app.logs.push(format!(
                    "Timestamp: {}",
                    Local::now().format("%Y%m%d-%H:%M:%S")
                ));

                let success = run_pre_release_checks(path, &mut app.logs, &app.platform);
                if success {
                    let _ = rename_previous_binary_if_exists(path, &mut app.logs, &app.platform);
                    app.logs
                        .push("Deploy local completado con éxito.".to_string());
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
