use std::time::Duration;
use crate::securelog::read_secure_log_formatted;
use crate::AvisaCtlApp;
use eframe::egui::{self, RichText};

pub fn logviewer_tab(app: &mut AvisaCtlApp, ctx: &egui::Context) {
    ctx.request_repaint_after(Duration::from_secs(1));
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Visor de Logs Combinados");
        ui.add_space(8.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_height(400.0);
            egui::ScrollArea::vertical()
                .id_salt("combined_log_scroll_area")
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let action_logs = app.logs.lock().unwrap();
                    let secure_logs = read_secure_log_formatted();

                    let mut combined: Vec<(String, String)> = Vec::new();

                    for l in action_logs.iter() {
                        combined.push(("[LOG]".to_string(), l.clone()));
                    }

                    for l in secure_logs {
                        combined.push(("[SECURE]".to_string(), l));
                    }

                    for (tag, line) in combined {
                        let rich_line = if tag == "[LOG]" {
                            RichText::new(format!("{tag} {line}"))
                                .size(16.0) // 🔍 Tamaño más grande para log de acciones
                                .strong()
                        } else {
                            RichText::new(format!("{tag} {line}")).size(12.0)
                        };

                        ui.label(rich_line);
                    }
                });
        });
    });
}
