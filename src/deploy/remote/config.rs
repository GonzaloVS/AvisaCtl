#[derive(Debug, Clone)]
pub struct RemoteConfig {
    pub server_address: String,
    pub username: String,
    pub pass: String,
    pub remote_path: String,
    pub secure_log_path: Option<String>,
}