use eframe::{egui::{self, TopBottomPanel}, App, Frame};
use crate::deploy::gui::deploy_tab;
use crate::config::{load_config, AvisaCtlConfig};

pub struct AvisaCtlApp {
    pub config: AvisaCtlConfig,
    pub current_tab: Tab,
    pub logs: Vec<String>,
    pub project_path: Option<String>,
    pub platform: crate::deploy::logic::Platform,
    pub target: crate::deploy::logic::DeployTarget,
    pub server_address: String,
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
            logs: vec![],
            project_path: None,
            platform: crate::deploy::logic::Platform::Linux,
            target: crate::deploy::logic::DeployTarget::Local,
            server_address: config.last_server_address.clone(),
        }
    }
}

impl App for AvisaCtlApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Deploy, "🚀 Deploy");
                ui.selectable_value(&mut self.current_tab, Tab::Backup, "💾 Backup");
                ui.selectable_value(&mut self.current_tab, Tab::Services, "📡 Servicios");
                ui.selectable_value(&mut self.current_tab, Tab::LogViewer, "📝 Logs");
            });
        });

        match self.current_tab {
            Tab::Deploy => deploy_tab(self, ctx),
            _ => {}
        }
    }
}
