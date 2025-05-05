use crate::securelog::logger::SecureLogger;
use crate::deploy::logic::RemoteConfig;
use serde_json::json;
use ssh2::Session;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub fn validate_remote_write_access(
    remote: &RemoteConfig,
    logger: &SecureLogger,
) -> bool {
    let addr = format!("{}:22", remote.server_address);
    let socket = match addr.to_socket_addrs().ok().and_then(|mut iter| iter.next()) {
        Some(s) => s,
        None => {
            logger.log_error("remote_resolve_failed", "No se pudo resolver dirección remota.");
            return false;
        }
    };

    match TcpStream::connect_timeout(&socket, Duration::from_secs(10)) {
        Ok(tcp) => {
            let mut session = Session::new().unwrap();
            session.set_tcp_stream(tcp);
            if session.handshake().is_err() {
                logger.log_error("remote_handshake_failed", "Falló handshake SSH.");
                return false;
            }

            if session.userauth_password(&remote.username, &remote.pass).is_err() {
                logger.log_error("remote_auth_failed", "Autenticación SSH fallida.");
                return false;
            }

            if !session.authenticated() {
                logger.log_error("remote_auth_not_verified", "No autenticado.");
                return false;
            }

            logger.log_event("remote_access_verified", json!({
                "server": remote.server_address,
                "path": remote.remote_path
            }));
            true
        }
        Err(e) => {
            logger.log_error("remote_connection_failed", &format!("Error conectando: {}", e));
            false
        }
    }
}
