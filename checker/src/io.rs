use crate::formula::CtlFormula;
use crate::kripke_structure::KripkeStructure;
use crate::parser::parse_ctl_formula;
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader}; // Adjust based on your actual path

/// Reads a file and parses each line into a CtlFormula.
/// Skips empty lines and lines starting with '#'.
pub fn load_formulas_from_file(path: &str) -> Result<Vec<CtlFormula>, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open formula file '{}': {}", path, e))?;

    let reader = BufReader::new(file);
    let mut formulas = Vec::new();

    for (index, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| format!("Error reading line {}: {}", index + 1, e))?;

        let trimmed = line.trim();

        // Skip empty lines or comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Parse the formula and wrap errors with line numbers for easier debugging
        match parse_ctl_formula(trimmed) {
            Ok(f) => formulas.push(f),
            Err(e) => return Err(format!("Line {}: {}", index + 1, e)),
        }
    }

    Ok(formulas)
}
pub fn load_model_from_prism(lab_path: &str, tra_path: &str) -> Result<KripkeStructure, String> {
    let mut structure = KripkeStructure::new();

    // 1. Parse Labels (.lab file)
    let lab_file = File::open(lab_path).map_err(|e| format!("Failed to open .lab file: {}", e))?;
    let mut lab_reader = BufReader::new(lab_file);
    let mut lab_lines = lab_reader.lines();

    // The first line contains the label mapping: 0="init" 1="deadlock" 2="p" ...
    let header = lab_lines
        .next()
        .ok_or("Empty .lab file")?
        .map_err(|e| e.to_string())?;

    let label_map = parse_label_header(&header);

    // Parse state lines: "0: 0 2" (State 0 has labels at index 0 and 2)
    for line_res in lab_lines {
        let line = line_res.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(':').collect();
        let state_id: usize = parts[0]
            .trim()
            .parse()
            .map_err(|_| "Invalid state ID in .lab")?;

        let label_indices: Vec<usize> = parts[1]
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        let mut state_labels = HashSet::new();
        let mut is_initial = false;

        for idx in label_indices {
            if let Some(label_name) = label_map.get(&idx) {
                if label_name == "init" {
                    is_initial = true;
                }
                state_labels.insert(label_name.clone());
            }
        }

        // Add state to the structure.
        // We assume PRISM state IDs are sequential starting from 0.
        structure.add_state(
            &format!("s{}", state_id),
            state_labels.into_iter().collect(),
            is_initial,
        );
    }

    // 2. Parse Transitions (.tra file)
    let tra_file = File::open(tra_path).map_err(|e| format!("Failed to open .tra file: {}", e))?;
    let tra_reader = BufReader::new(tra_file);
    let mut tra_lines = tra_reader.lines();

    // Skip header: "num_states num_transitions"
    tra_lines.next();

    for line_res in tra_lines {
        let line = line_res.map_err(|e| e.to_string())?;
        let nums: Vec<usize> = line
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        if nums.len() >= 2 {
            // Add transition from source to target
            // Using NodeIndex::new because we trust PRISM's sequential IDs
            structure.add_transition(NodeIndex::new(nums[0]), NodeIndex::new(nums[1]));
        }
    }

    Ok(structure)
}

/// Helper to parse PRISM header: 0="init" 1="deadlock" 2="p"
fn parse_label_header(header: &str) -> HashMap<usize, String> {
    let mut map = HashMap::new();
    // Simple split-based parser for the PRISM header format
    for part in header.split_whitespace() {
        if let Some((id_str, name_with_quotes)) = part.split_once('=') {
            if let Ok(id) = id_str.parse::<usize>() {
                let name = name_with_quotes.replace('\"', "");
                map.insert(id, name);
            }
        }
    }
    map
}
