use eframe::egui::{Ui, RichText};
use native_dialog::FileDialog;
use crate::app::AvisaCtlApp;

pub fn render_project_selector(app: &mut AvisaCtlApp, ui: &mut Ui) {
    ui.group(|ui| {
        ui.label(RichText::new("Proyecto").strong());
        ui.horizontal(|ui| {
            if ui.button("Seleccionar carpeta").clicked() {
                if let Some(path) = FileDialog::new().show_open_single_dir().ok().flatten() {
                    app.project_path = Some(path.display().to_string());
                }
            }

            ui.label(app.project_path.clone().unwrap_or_else(|| "No seleccionado.".into()));
        });
    });

    ui.add_space(10.0);
}
