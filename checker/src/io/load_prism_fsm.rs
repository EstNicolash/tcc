use crate::core::kripke_structure::KripkeStructure;
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};

pub fn load_prism_fsm(lab_path: &str, tra_path: &str) -> Result<KripkeStructure, String> {
    let mut structure = KripkeStructure::new();
    let mut label_names: HashMap<usize, String> = HashMap::new();

    let lab_file = File::open(lab_path).map_err(|e| e.to_string())?;
    //let mut lab_reader = BufReader::new(lab_file);
    let lab_reader = BufReader::new(lab_file);
    let mut lab_lines = lab_reader.lines();

    if let Some(Ok(header)) = lab_lines.next() {
        for part in header.split_whitespace() {
            if let Some((id_str, name_with_quotes)) = part.split_once('=') {
                if let Ok(id) = id_str.parse::<usize>() {
                    label_names.insert(id, name_with_quotes.replace('\"', ""));
                }
            }
        }
    }

    for (line_idx, line_res) in lab_lines.enumerate() {
        let line = line_res.map_err(|e| e.to_string())?;
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 2 {
            continue;
        }

        let mut labels = HashSet::new();
        let mut is_init = false;
        for idx in parts[1]
            .split_whitespace()
            .filter_map(|s| s.parse::<usize>().ok())
        {
            if let Some(name) = label_names.get(&idx) {
                if name == "init" {
                    is_init = true;
                }
                labels.insert(name.clone());
            }
        }

        let labels_vec: Vec<String> = labels.into_iter().collect();
        structure.add_state(&format!("s{}", line_idx), labels_vec, is_init);
    }

    let tra_file = File::open(tra_path).map_err(|e| e.to_string())?;
    let tra_reader = BufReader::new(tra_file);
    for line in tra_reader.lines().skip(1).filter_map(|l| l.ok()) {
        let nums: Vec<usize> = line
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if nums.len() >= 2 {
            structure.add_transition(NodeIndex::new(nums[0]), NodeIndex::new(nums[1]));
        }
    }
    Ok(structure)
}
