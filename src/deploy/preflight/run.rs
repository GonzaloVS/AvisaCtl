use crate::deploy::logic::Platform;
use crate::securelog::logger::SecureLogger;
use crate::deploy::docker::{
    ensure::ensure_dockerfile_exists,
    build::build_with_docker,
};
use crate::deploy::preflight::{
    cargo::run_cargo_steps,
    checks::{run_unwrap_check, run_lockfile_check, run_bin_size_check},
};
use crate::deploy::preflight::options::PreflightOptions;


pub async fn run_preflight(
    project_path: &str,
    logs: &mut Vec<String>,
    platform: &Platform,
    logger: &SecureLogger,
    opts: &PreflightOptions,
) -> bool {
    logs.push("Iniciando preflight...".to_string());

    if !run_cargo_steps(
        project_path,
        logs,
        logger,
        opts.check_format,
        opts.check_warnings,
        opts.check_tests,
        opts.check_audit,
    )
        .await
    {
        return false;
    }

    if opts.check_unwraps && !run_unwrap_check(project_path, logs, logger) {
        return false;
    }

    if !run_lockfile_check(project_path, logs, logger) {
        return false;
    }

    if opts.check_bin_size && !run_bin_size_check(
        project_path,
        platform,
        opts.max_bin_size,
        logs,
        logger,
    ) {
        return false;
    }

    if !ensure_dockerfile_exists(project_path, logs, logger) {
        return false;
    }

    logs.push("Preflight finalizado. Construyendo Docker...".to_string());
    build_with_docker(project_path, logs, logger).await
}
