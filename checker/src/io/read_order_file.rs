use colored::*;
/// Reads a NuSMV-style .ord file and returns a list of variable/bit names.
pub fn read_order_file(path: &str) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.to_string())
            .collect(),
        Err(e) => {
            eprintln!("{} Failed to read order file: {}", "Warning:".yellow(), e);
            Vec::new()
        }
    }
}
