use eframe::egui::{self, Ui, Margin, RichText};
use crate::app::AvisaCtlApp;
use crate::deploy::gui::secure_log_verification::verify_or_prompt_secure_log;

pub fn render_header(app: &mut AvisaCtlApp, ui: &mut Ui) {
    ui.heading("Deploy");
    ui.add_space(8.0);

    // Validación del secure.log si aún no se hizo
    verify_or_prompt_secure_log(app, ui);

    // Mostrar mensaje de estado
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

        ui.add_space(8.0);
    }
}
