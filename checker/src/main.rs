mod algorithms;
mod cli;
mod core;
mod io;
mod modeling;
mod specs;

use clap::Parser;
use cli::{Algorithm, Cli, Commands, InputFormat};
use colored::*;
use std::fs;
use std::process;

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Verify {
            model_path,
            spec_path,
            format,
            algorithm,
        } => {
            run_verification(model_path, spec_path, format, algorithm);
        }

        Commands::TestParser { input_file, output } => {
            run_parser_test(input_file, output);
        }
    }
}

fn run_verification(
    model_path: String,
    spec_path: String,
    format: InputFormat,
    algorithm: Algorithm,
) {
    println!(
        "{}",
        format!("--- Model Checker (Verify): {} ---", model_path.bold()).blue()
    );

    let model = match format {
        InputFormat::Pnml => {
            let fsm_path = model_path.replace(".pnml", ".fsm");
            io::load_pnml_fsm::load_pnml_fsm(&fsm_path, &model_path)
        }
        InputFormat::Prism => {
            let fsm_path = model_path.replace(".prism", ".fsm");
            io::load_prism_fsm::load_prism_fsm(&fsm_path, &model_path)
        }

        InputFormat::Ssmv => {
            unimplemented!(
                "Ssmv expansion to Kripke structure is not yet integrated into the Verify flow."
            );
        }
    }
    .unwrap_or_else(|e| {
        eprintln!("{} {}", "Error loading model:".red().bold(), e);
        process::exit(1);
    });

    let formulas = io::load_formulas::load_formulas(&spec_path).unwrap_or_else(|e| {
        eprintln!("{} {}", "Error loading formulas:".red().bold(), e);
        process::exit(1);
    });

    println!(
        "{} Model loaded via {:?} ({} states). {} formulas found.\n",
        "✔".green(),
        format,
        model.graph.node_count(),
        formulas.len()
    );

    for (i, phi) in formulas.iter().enumerate() {
        let is_valid = match algorithm {
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

fn run_parser_test(input_file: String, output: Option<String>) {
    println!("{}", "--- Mode: SSMV Parser Test ---".yellow().bold());

    let input_content = fs::read_to_string(&input_file).unwrap_or_else(|e| {
        eprintln!(
            "{} Error reading file {}: {}",
            "Error:".red().bold(),
            input_file,
            e
        );
        process::exit(1);
    });

    match modeling::ssmv_parser::parse_ssmv(&input_content) {
        Ok(model) => {
            let result = format!("{}", model);

            if let Some(out_path) = output {
                fs::write(&out_path, &result).unwrap_or_else(|e| {
                    eprintln!(
                        "{} Error writing to {}: {}",
                        "Error:".red().bold(),
                        out_path,
                        e
                    );
                    process::exit(1);
                });
                println!("{} Success! Result saved to: {}", "✔".green(), out_path);
            } else {
                println!("{} Success! Result:\n", "✔".green());
                println!("{}", result);
            }
        }
        Err(e) => {
            eprintln!("{} {}", "Parser Error:".red().bold(), e);
            process::exit(1);
        }
    }
}
