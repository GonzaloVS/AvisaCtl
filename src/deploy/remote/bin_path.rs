use std::fs;
use std::path::Path;

pub fn extract_package_name(cargo_toml_path: &Path) -> Option<String> {
    let contents = fs::read_to_string(cargo_toml_path).ok()?;
    let mut inside_package = false;

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("[package]") {
            inside_package = true;
        } else if inside_package && line.starts_with("name") {
            return line
                .split('=')
                .nth(1)
                .map(|s| s.trim().trim_matches('"').to_string());
        }
    }

    None
}