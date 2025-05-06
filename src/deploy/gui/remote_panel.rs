use eframe::egui;
use eframe::egui::{Ui, RichText, TextEdit};
use crate::app::AvisaCtlApp;

pub fn render_remote_panel(app: &mut AvisaCtlApp, ui: &mut Ui) {
    ui.group(|ui| {
        ui.label(RichText::new("Destino").strong());

        ui.add_space(4.0);

        egui::Grid::new("remote_panel_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .min_col_width(80.0)
            .show(ui, |ui| {
                ui.label("Servidor:");
                ui.text_edit_singleline(&mut app.server_address);
                ui.end_row();

                ui.label("Usuario:");
                ui.text_edit_singleline(&mut app.remote_user);
                ui.end_row();

                ui.label("Password:");
                ui.add(TextEdit::singleline(&mut app.remote_pass).password(true));
                ui.end_row();

                ui.label("Ruta remota:");
                ui.text_edit_singleline(&mut app.remote_path);
                ui.end_row();

                ui.label("Plataforma:");
                ui.label("Linux");
                ui.end_row();
            });
    });
}
