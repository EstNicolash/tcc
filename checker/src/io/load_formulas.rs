use crate::specs::ctl_formula::CtlFormula;
use crate::specs::ctl_parser::parse_ctl_formula;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn load_formulas(path: &str) -> Result<Vec<CtlFormula>, String> {
    let file = File::open(path).map_err(|e| format!("Error opening .spec: {}", e))?;
    let reader = BufReader::new(file);
    let mut formulas = Vec::new();

    for line in reader.lines().filter_map(|l| l.ok()) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Ok(f) = parse_ctl_formula(trimmed) {
            formulas.push(f);
        }
    }
    Ok(formulas)
}
