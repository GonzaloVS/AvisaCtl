mod app;
mod backup;
mod config;
mod deploy;
mod securelog;
mod logview;
mod services;

use eframe::{egui, run_native, NativeOptions};
use eframe::egui::ViewportBuilder;
use crate::app::AvisaCtlApp;
//#[tokio::main]
// async fn main() -> eframe::Result<()> {
//     let options = NativeOptions::default();
//     run_native(
//         "AvisaCtl",
//         options,
//         Box::new(|_cc| Ok(Box::new(app::AvisaCtlApp::default()))),
//     )
// }


#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    let native_options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            //.with_min_inner_size([800.0, 600.0])
            .with_resizable(true),
        ..Default::default()
    };

    run_native(
        "AvisaCtl",
        native_options,
        Box::new(|_cc| Ok(Box::new(AvisaCtlApp::default()))),
    )
}

