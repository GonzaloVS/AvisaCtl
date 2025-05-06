#[cfg(target_os = "windows")]
pub fn convert_windows_path_for_docker(path: &str) -> String {
    let drive_letter = &path[0..1].to_lowercase();
    let without_colon = path[2..].replace("\\", "/");
    format!("/{}/{}", drive_letter, without_colon)
}

#[cfg(not(target_os = "windows"))]
pub fn convert_windows_path_for_docker(path: &str) -> String {
    path.to_string()
}