use crate::app::AvisaCtlApp;
use eframe::egui::{self, RichText, Ui};

pub fn render_logs_panel(app: &mut AvisaCtlApp, ui: &mut Ui) {
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
}
