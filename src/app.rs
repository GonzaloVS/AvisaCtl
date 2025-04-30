use eframe::{
    egui::{self, TopBottomPanel},
    App, Frame,
};
use std::sync::{Arc, Mutex};

use crate::config::{load_config, AvisaCtlConfig};
use crate::deploy::gui::deploy_tab;
use crate::logview::gui::logviewer_tab;

pub struct AvisaCtlApp {
    pub config: AvisaCtlConfig,
    pub current_tab: Tab,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub project_path: Option<String>,
    pub platform: crate::deploy::logic::Platform,
    pub server_address: String,
    pub remote_user: String,
    pub remote_pass: String,
    pub remote_path: String,
    //pub secure_log_path: Option<String>,
    pub is_deploying: bool,
    pub cancel_deploy: bool,
    pub log_validated: bool,
    pub log_valid: Option<bool>,
    // Validaciones del panel de configuración
    pub check_format: bool,
    pub check_warnings: bool,
    pub check_tests: bool,
    pub check_audit: bool,
    pub check_unwraps: bool,
    pub check_bin_size: bool,
    pub max_bin_size: u64,
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
            server_address: config.last_server_address.clone(),
            remote_user: config.last_remote_user.clone(),
            remote_pass: config.last_remote_pass.clone(),
            remote_path: config.last_remote_path.clone(),
            is_deploying: false,
            cancel_deploy: false,
            log_validated: false,
            log_valid: None,
            //panel de configuración
            check_format: config.check_format.clone(),
            check_warnings: config.check_warnings.clone(),
            check_tests: config.check_tests.clone(),
            check_audit: config.check_audit.clone(),
            check_unwraps: config.check_unwraps.clone(),
            check_bin_size: config.check_bin_size.clone(),
            max_bin_size: config.max_bin_size.clone(),
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

        match self.current_tab {
            Tab::Deploy => { deploy_tab(self, ctx); }
            Tab::Backup => {}
            Tab::Services => {}
            Tab::LogViewer => { logviewer_tab(self, ctx)}
        }
    }
}
