use crate::core::kripke_structure::KripkeStructure;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn load_pnml_fsm(fsm_path: &str, _pnml_path: &str) -> Result<KripkeStructure, String> {
    let file = File::open(fsm_path).map_err(|e| format!("Error opening .fsm: {}", e))?;
    let reader = BufReader::new(file);

    let mut structure = KripkeStructure::new();
    let mut state_variables = Vec::new();
    let mut section = 0;
    let mut nodes = Vec::new();

    for line_res in reader.lines() {
        let line = line_res.map_err(|e| e.to_string())?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "---" {
            section += 1;
            continue;
        }

        match section {
            0 => {
                let var_name = trimmed
                    .split('(')
                    .next()
                    .unwrap_or(trimmed)
                    .trim()
                    .to_string();
                state_variables.push(var_name);
            }
            1 => {
                let values: Vec<i32> = trimmed
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                let mut labels = HashSet::new();
                for (idx, &val) in values.iter().enumerate() {
                    if idx < state_variables.len() {
                        let name = &state_variables[idx];
                        labels.insert(format!("{}={}", name, val));

                        if val > 0 {
                            labels.insert(name.clone());
                        }
                    }
                }

                let current_id = nodes.len();
                let node_idx = structure.add_state(
                    &format!("s{}", current_id),
                    labels.into_iter().collect(),
                    current_id == 0,
                );
                nodes.push(node_idx);
            }
            2 => {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();

                if parts.len() >= 2 {
                    let raw_src: usize = parts[0].parse().unwrap_or(0);
                    let raw_dst: usize = parts[1].parse().unwrap_or(0);

                    if raw_src == 0 || raw_dst == 0 {
                        return Err(format!(
                            "Error: ID of state 0 detected in transition. The parser expects base 1."
                        ));
                    }

                    let src_id = raw_src - 1;
                    let dst_id = raw_dst - 1;

                    while src_id >= nodes.len() || dst_id >= nodes.len() {
                        let new_id = nodes.len();
                        let n = structure.add_state(&format!("s{}", new_id), vec![], new_id == 0);
                        nodes.push(n);
                    }

                    structure.add_transition(nodes[src_id], nodes[dst_id]);
                }
            }
            _ => break,
        }
    }

    if nodes.is_empty() {
        return Err("The .fsm file appears to be empty or malformed.".to_string());
    }

    println!(
        "✔ Structure loaded: {} states, {} transitions.",
        structure.graph.node_count(),
        structure.graph.edge_count()
    );
    structure.make_total();
    Ok(structure)
}
