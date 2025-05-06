use eframe::egui::{self, Ui, RichText};
use crate::{app::AvisaCtlApp, config::save_config};

pub fn render_config_panel(app: &mut AvisaCtlApp, ui: &mut Ui) {
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

            ui.label("\nValidaciones avanzadas:");
            if ui.checkbox(&mut app.config.check_unwraps, "unwraps").changed() {
                let _ = save_config(&app.config);
            }

            ui.horizontal(|ui| {
                if ui.checkbox(&mut app.config.check_bin_size, "bin size").changed() {
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
}
