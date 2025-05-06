use crate::app::AvisaCtlApp;
use eframe::egui::{Ui};
use native_dialog::FileDialog;
use std::path::Path;
use crate::config::save_config;

pub fn verify_or_prompt_secure_log(app: &mut AvisaCtlApp, _ui: &mut Ui) {
    if app.log_validated {
        return;
    }

    app.log_validated = true;

    let log_path = app.config.secure_log_path.clone().unwrap_or_default();
    let log_exists = Path::new(&log_path).exists();

    if log_path.is_empty() || !log_exists {
        if let Some(folder) = FileDialog::new()
            .set_title("Selecciona la carpeta donde guardar secure.log")
            .show_open_single_dir()
            .ok()
            .flatten()
        {
            let mut new_config = app.config.clone();
            let path_str = folder.display().to_string();
            new_config.secure_log_path = Some(path_str.clone());

            if save_config(&new_config).is_ok() {
                app.config = new_config;
                app.logs.log(format!("Ruta de secure.log guardada: {}", path_str));
            } else {
                app.logs.log("Error guardando configuración.");
                app.log_valid = Some(false);
                return;
            }
        } else {
            app.logs
                .log("No se seleccionó una carpeta para el secure.log.");
            app.log_valid = Some(false);
            return;
        }
    }

    let result = crate::securelog::ensure_secure_log_initialized_at(
        app.config.secure_log_path.as_ref().unwrap(),
    );
    if let Err(e) = result {
        app.logs
            .log(format!("Error al inicializar secure.log: {e}"));
        app.log_valid = Some(false);
        return;
    }

    match crate::securelog::validate_secure_log_integrity_at(
        app.config.secure_log_path.as_ref().unwrap(),
    ) {
        Ok(_) => {
            app.logs.log("secure.log verificado: íntegro.");
            app.log_valid = Some(true);
        }
        Err(e) => {
            app.logs
                .log(format!("secure.log corrupto: {e}"));
            app.log_valid = Some(false);
        }
    }
}

// pub fn verify_or_prompt_secure_log(app: &mut AvisaCtlApp, ui: &mut Ui) {
//     if !app.log_validated {
//         app.log_validated = true;
//
//         let log_path = app.config.secure_log_path.clone().unwrap_or_default();
//         let log_exists = Path::new(&log_path).exists();
//
//         if log_path.is_empty() || !log_exists {
//             if let Some(folder) = FileDialog::new()
//                 .set_title("Selecciona la carpeta donde guardar secure.log")
//                 .show_open_single_dir()
//                 .ok()
//                 .flatten()
//             {
//                 let mut new_config = app.config.clone();
//                 let path_str = folder.display().to_string();
//                 new_config.secure_log_path = Some(path_str.clone());
//
//                 if save_config(&new_config).is_ok() {
//                     app.config = new_config;
//                     app.logs.log(format!("Ruta de secure.log guardada: {}", path_str));
//                 } else {
//                     app.logs.log("Error guardando configuración.");
//                     app.log_valid = Some(false);
//                     return;
//                 }
//             } else {
//                 app.logs.log("No se seleccionó una carpeta para el secure.log.");
//                 app.log_valid = Some(false);
//                 return;
//             }
//         }
//
//         let result = crate::securelog::ensure_secure_log_initialized_at(
//             app.config.secure_log_path.as_ref().unwrap(),
//         );
//         if let Err(e) = result {
//             app.logs.log(format!("Error al inicializar secure.log: {e}"));
//             app.log_valid = Some(false);
//             return;
//         }
//
//         match crate::securelog::validate_secure_log_integrity_at(
//             app.config.secure_log_path.as_ref().unwrap(),
//         ) {
//             Ok(_) => {
//                 app.logs.log("secure.log verificado: íntegro.");
//                 app.log_valid = Some(true);
//             }
//             Err(e) => {
//                 app.logs.log(format!("secure.log corrupto: {e}"));
//                 app.log_valid = Some(false);
//             }
//         }
//     }
//
//     // if let Some(valid) = app.log_valid {
//     //     let color = if valid {
//     //         egui::Color32::DARK_GREEN
//     //     } else {
//     //         egui::Color32::RED
//     //     };
//     //     let text = if valid {
//     //         "secure.log verificado: íntegro"
//     //     } else {
//     //         "ALERTA: secure.log alterado o corrupto"
//     //     };
//     //
//     //     Frame::default()
//     //         .fill(color)
//     //         .inner_margin(Margin::symmetric(10, 6))
//     //         .show(ui, |ui| {
//     //             ui.label(RichText::new(text).strong().color(egui::Color32::WHITE));
//     //         });
//     // }
// }
