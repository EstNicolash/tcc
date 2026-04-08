mod algorithms;
mod cli;
mod core;
mod io;
mod modeling;
mod specs;

use clap::Parser;
use cli::{Algorithm, Args, InputFormat};
use colored::*;
fn main() {
    let args = Args::parse();

    println!(
        "{}",
        format!("--- Model Checker: {} ---", args.model_path.bold()).blue()
    );

    let model = match args.format {
        InputFormat::Pnml => {
            let fsm_path = args.model_path.replace(".pnml", ".fsm");
            io::load_pnml_fsm::load_pnml_fsm(&fsm_path, &args.model_path)
        }
        InputFormat::Prism => {
            let fsm_path = args.model_path.replace(".prism", ".fsm");
            io::load_prism_fsm::load_prism_fsm(&fsm_path, &args.model_path)
        }
    }
    .unwrap_or_else(|e| {
        eprintln!("{} {}", "Error loading model:".red().bold(), e);
        std::process::exit(1);
    });

    let formulas = io::load_formulas::load_formulas(&args.spec_path).unwrap_or_else(|e| {
        eprintln!("{} {}", "Error loading formulas:".red().bold(), e);
        std::process::exit(1);
    });

    println!(
        "{} Model loaded via {:?} ({} states). {} formulas found.\n",
        "✔".green(),
        args.format,
        model.graph.node_count(),
        formulas.len()
    );

    for (i, phi) in formulas.iter().enumerate() {
        let is_valid = match args.algorithm {
            Algorithm::Labelling => algorithms::labelling::verify(&model, phi),
        };

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
