use crate::{app::AvisaCtlApp, securelog::read_secure_log_formatted};
use eframe::egui::{self, RichText, Ui};
use std::path::PathBuf;

pub fn render_secure_log_panel(app: &mut AvisaCtlApp, ui: &mut Ui) {
    ui.add_space(12.0);
    ui.separator();

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
}
