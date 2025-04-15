use std::fs;
use std::path::Path;

/// Lista de funciones/patrones peligrosos relacionados con unwraps
const UNWRAP_PATTERNS: &[&str] = &[
    ".unwrap()",
    ".expect(",
    "unwrap_err()",
    "expect_err(",
    ".unwrap_or(",
    ".unwrap_or_else(",
];

/// Revisa un archivo `.rs` y devuelve una lista de problemas encontrados
fn analyze_file(file_path: &Path, content: &str) -> Vec<String> {
    let mut results = vec![];

    for (i, line) in content.lines().enumerate() {
        for pattern in UNWRAP_PATTERNS {
            if line.contains(pattern) && !line.contains("// safe") {
                results.push(format!(
                    "{}:{} contiene '{}'",
                    file_path.display(),
                    i + 1,
                    pattern
                ));
            }
        }
    }

    results
}

/// Función principal: recorre el código fuente y busca usos no justificados de unwraps
pub fn check_no_dangerous_patterns(project_path: &Path) -> Result<(), Vec<String>> {
    let mut problems = vec![];
    let src_path = project_path.join("src");

    fn recurse(path: &Path, acc: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    recurse(&path, acc);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        acc.extend(analyze_file(&path, &content));
                    }
                }
            }
        }
    }

    recurse(&src_path, &mut problems);

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}
