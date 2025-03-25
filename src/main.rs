mod app;
mod backup;
mod config;
mod deploy;
mod logview;
mod services;

use eframe::{run_native, NativeOptions};

fn main() -> eframe::Result<()> {
    let options = NativeOptions::default();
    run_native(
        "AvisaCtl",
        options,
        Box::new(|_cc| Ok(Box::new(app::AvisaCtlApp::default()))),
    )
}
