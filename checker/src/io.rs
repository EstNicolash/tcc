use crate::formula::CtlFormula;
use crate::model::Model;
use crate::parser::parse_ctl_formula;
//use petgraph::graph::NodeIndex;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};

// Load a model from an .fsm file of ltsmin pnml
pub fn load_model_from_fsm(fsm_path: &str, _pnml_path: &str) -> Result<Model, String> {
    let file = File::open(fsm_path).map_err(|e| format!("Erro ao abrir .fsm: {}", e))?;
    let reader = BufReader::new(file);

    let mut structure = Model::new();
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
                            "Erro: ID de estado 0 detectado na transição. O parser espera base 1."
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
        return Err("O arquivo .fsm parece estar vazio ou mal formatado.".to_string());
    }

    println!(
        "✔ Modelo carregado: {} estados, {} transições.",
        structure.graph.node_count(),
        structure.graph.edge_count()
    );
    structure.make_total();
    Ok(structure)
}
pub fn load_formulas_from_file(path: &str) -> Result<Vec<CtlFormula>, String> {
    let file = File::open(path).map_err(|e| format!("Erro ao abrir .spec: {}", e))?;
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
/*
pub fn load_model_from_prism(lab_path: &str, tra_path: &str) -> Result<Model, String> {
    let mut structure = Model::new();
    let mut label_names: HashMap<usize, String> = HashMap::new();

    let lab_file = File::open(lab_path).map_err(|e| e.to_string())?;
    let mut lab_reader = BufReader::new(lab_file);
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
}*/
