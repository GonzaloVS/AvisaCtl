use crate::app::AvisaCtlApp;
use eframe::egui::{self, Context};
use crate::deploy::gui::header::render_header;
use super::config_panel::render_config_panel;
use super::deploy_button::render_deploy_button;
use super::logs_panel::render_logs_panel;
use super::project_selector::render_project_selector;
use super::remote_panel::render_remote_panel;
use super::secure_log_panel::render_secure_log_panel;


pub fn deploy_tab(app: &mut AvisaCtlApp, ctx: &Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // ui.heading("Deploy");
        // ui.add_space(8.0);
        //
        // verify_or_prompt_secure_log(app, ui);
        // if app.log_valid != Some(true) {
        //     return;
        // }

        render_header(app, ui);
        if app.log_valid != Some(true) {
            return;
        }


        render_project_selector(app, ui);

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            render_remote_panel(app, ui);
            ui.add_space(10.0);
            render_config_panel(app, ui);
        });

        ui.add_space(10.0);

        render_deploy_button(app, ui);
        render_logs_panel(app, ui);
        render_secure_log_panel(app, ui);
    });
}
