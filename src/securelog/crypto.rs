use std::process::Command;
use std::io::Write;

pub fn sign_with_gpg(data: &str) -> Result<String, String> {
    let output = Command::new("gpg")
        .arg("--sign")
        .arg("--armor")
        .arg("--batch")
        .arg("--yes")
        .arg("--local-user")
        .arg("avisactl@localhost")
        .arg("--output")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(data.as_bytes())?;
            }
            let output = child.wait_with_output()?;
            Ok(output)
        })
        .map_err(|e| format!("Error ejecutando GPG: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn fetch_tsa_timestamp(hash: &str) -> Result<String, String> {
    Ok(format!("TSA:{}:{}", chrono::Utc::now().to_rfc3339(), hash))
}
