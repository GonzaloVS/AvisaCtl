mod app;
mod backup;
mod checks;
mod config;
mod deploy;
mod logview;
mod securelog;
mod services;

use crate::app::AvisaCtlApp;
use eframe::egui::ViewportBuilder;
use eframe::{run_native, NativeOptions};

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let native_options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_resizable(true),
        ..Default::default()
    };

    run_native(
        "AvisaCtl",
        native_options,
        Box::new(|_cc| Ok(Box::new(AvisaCtlApp::default()))),
    )
}
