use crate::{
    app::AvisaCtlApp,
    deploy::{
        remote::{
            config::RemoteConfig,
            deploy::deploy_to_remote_async,
        },
        preflight::{
            options::PreflightOptions,
            remote_validation::validate_remote_write_access,
            run::run_preflight},
    },
    securelog::logger::SecureLogger,
};
use chrono::Utc;
use eframe::egui::{Button, RichText, Ui};
use std::sync::{Arc, Mutex};

pub fn render_deploy_button(app: &mut AvisaCtlApp, ui: &mut Ui) {
    if !*app.is_deploying_flag.lock().unwrap() {
        if ui
            .add_sized(
                [220.0, 44.0], // tamaño del botón
                Button::new(
                    RichText::new("▶ Iniciar Deploy")
                        .strong()    // texto en negrita
                        .size(18.0), // tamaño de fuente en puntos
                ),
            )
            .clicked()
        {

            app.logs.get().lock().unwrap().clear();
            *app.is_deploying_flag.lock().unwrap() = true;
            app.cancel_deploy = false;

            let secure_logger = Arc::new(SecureLogger::new());

            if let Some(project_path) = &app.project_path {
                let preflight_opts = PreflightOptions {
                    check_format: app.config.check_format,
                    check_warnings: app.config.check_warnings,
                    check_tests: app.config.check_tests,
                    check_audit: app.config.check_audit,
                    check_unwraps: app.config.check_unwraps,
                    check_bin_size: app.config.check_bin_size,
                    max_bin_size: app.config.max_bin_size,
                };

                let remote_cfg = RemoteConfig {
                    server_address: app.server_address.clone(),
                    username: app.remote_user.clone(),
                    pass: app.remote_pass.clone(),
                    remote_path: app.remote_path.clone(),
                    secure_log_path: app.config.secure_log_path.clone(),
                };

                let logs_arc = app.logs.clone();
                let cancel_flag = Arc::new(Mutex::new(app.cancel_deploy));
                let config = app.config.clone();
                let is_deploying_flag = Arc::clone(&app.is_deploying_flag); // crea un Arc<Mutex<bool>> en tu struct si no existe

                tokio::spawn({
                    let path = project_path.clone();
                    let platform = app.platform.clone();
                    let secure_logger = Arc::clone(&secure_logger);
                    let cancel_flag_clone = Arc::clone(&cancel_flag);
                    let callback_logs = logs_arc.get();

                    async move {
                        logs_arc.log("Iniciando validaciones...");
                        logs_arc.log("Validando acceso al servidor remoto...");

                        if !validate_remote_write_access(&remote_cfg, &secure_logger) {
                            logs_arc.get().lock().unwrap().push((
                                Utc::now(),
                                "No se pudo establecer conexión remota SSH. Revisa el servidor, usuario o contraseña.".into(),
                            ));
                            *is_deploying_flag.lock().unwrap() = false;
                            return;
                        }

                        logs_arc.log("Acceso remoto verificado.");

                        let mut temp_logs = Vec::new();

                        if run_preflight(
                            &path,
                            &mut temp_logs,
                            &platform,
                            &secure_logger,
                            &preflight_opts,
                        )
                            .await
                        {
                            for line in temp_logs.drain(..) {
                                logs_arc.get().lock().unwrap().push((Utc::now(), line));
                            }

                            deploy_to_remote_async(
                                path,
                                platform,
                                remote_cfg,
                                config,
                                move |success| {
                                    let mut logs = callback_logs.lock().unwrap();
                                    if success {
                                        logs.push((Utc::now(), "Deploy remoto completado con éxito.".into()));
                                    } else {
                                        logs.push((Utc::now(), "Error durante el deploy remoto.".into()));
                                    }
                                    logs.push((Utc::now(), "Estado: deploy finalizado.".into()));
                                    *is_deploying_flag.lock().unwrap() = false;
                                },
                                cancel_flag_clone,
                                secure_logger,
                            );

                        } else {
                            for line in temp_logs.drain(..) {
                                logs_arc.get().lock().unwrap().push((Utc::now(), line));
                            }
                            logs_arc
                                .get()
                                .lock()
                                .unwrap()
                                .push((Utc::now(), "Se detuvo el deploy por error previo.".into()));
                        }
                    }
                });
            }
        }
    } else if ui.button("Cancelar Deploy").clicked() {
        app.cancel_deploy = true;
        app.logs.log("Cancelación solicitada.");
    }
}
