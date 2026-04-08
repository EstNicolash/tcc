mod formula;
mod io;
mod labelling;
mod model;
mod parser;

use colored::*;
use labelling::verify;

fn main() {
    // 1. Configuration and Paths
    let folder = "examples";
    let base_name = "FMS2";

    let fsm_path = format!("{}/{}.fsm", folder, base_name);
    let pnml_path = format!("{}/{}.pnml", folder, base_name);
    let spec_path = format!("{}/{}.spec", folder, base_name);

    println!(
        "{}",
        format!("--- Model Checker (FSM Mode): {} ---", base_name)
            .bold()
            .blue()
    );
    println!("Reading files from: {}/\n", folder.cyan());

    // 2. Load the Model using both FSM (graph) and PNML (metadata)
    // Updated to pass two arguments
    let model = match io::load_model_from_fsm(&fsm_path, &pnml_path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{} {}", "Model error:".red().bold(), e);
            return;
        }
    };

    // 3. Load Specifications (CTL Formulas)
    let formulas = match io::load_formulas_from_file(&spec_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{} {}", "Formula error:".red().bold(), e);
            return;
        }
    };

    println!(
        "{} Graph with {} states and {} formulas loaded.",
        "✔".green(),
        model.graph.node_count(),
        formulas.len()
    );
    println!("--------------------------------------------------\n");

    // 4. Verification Loop
    for (i, phi) in formulas.iter().enumerate() {
        let is_valid = verify(&model, phi);

        let result_text = if is_valid {
            "TRUE".green().bold()
        } else {
            "FALSE".red().bold()
        };

        println!(
            "{:>2}. [{}] {}\n    └─ Result: {}",
            i + 1,
            "CTL".yellow(),
            phi,
            result_text
        );
    }
}
