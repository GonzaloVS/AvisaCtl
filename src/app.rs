use eframe::{
    egui::{self, TopBottomPanel},
    App, Frame,
};
use std::sync::{Arc, Mutex};

use crate::config::{load_config, AvisaCtlConfig};
use crate::deploy::gui::deploy_tab;

pub struct AvisaCtlApp {
    pub config: AvisaCtlConfig,
    pub current_tab: Tab,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub project_path: Option<String>,
    pub platform: crate::deploy::logic::Platform,
    pub target: crate::deploy::logic::DeployTarget,
    pub server_address: String,
    pub remote_user: String,
    pub remote_pass: String,
    pub remote_path: String,
    pub is_deploying: bool,
    pub cancel_deploy: bool,
}

#[derive(PartialEq)]
pub enum Tab {
    Deploy,
    Backup,
    Services,
    LogViewer,
}

impl Default for AvisaCtlApp {
    fn default() -> Self {
        let config = load_config();
        Self {
            config: config.clone(),
            current_tab: Tab::Deploy,
            logs: Arc::new(Mutex::new(vec![])),
            project_path: if config.last_local_path.is_empty() {
                None
            } else {
                Some(config.last_local_path.clone())
            },
            platform: crate::deploy::logic::Platform::Linux,
            target: crate::deploy::logic::DeployTarget::Remote,
            server_address: config.last_server_address.clone(),
            remote_user: config.last_remote_user.clone(),
            remote_pass: config.last_remote_pass.clone(),
            remote_path: config.last_remote_path.clone(),
            is_deploying: false,
            cancel_deploy: false,
        }
    }
}

impl App for AvisaCtlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Deploy, "Deploy");
                ui.selectable_value(&mut self.current_tab, Tab::Backup, "Backup");
                ui.selectable_value(&mut self.current_tab, Tab::Services, "Servicios");
                ui.selectable_value(&mut self.current_tab, Tab::LogViewer, "Logs");
            });
        });

        if self.current_tab == Tab::Deploy {
            deploy_tab(self, ctx);
        }
    }
}
