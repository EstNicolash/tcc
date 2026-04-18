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
    _spec_path: String,
    format: InputFormat,
    algorithm: Algorithm,
) {
    println!(
        "{}",
        format!("--- Model Checker (Verify): {} ---", model_path.bold()).blue()
    );

    let input_content = fs::read_to_string(&model_path).unwrap_or_else(|e| {
        eprintln!("{} {}", "Error reading file:".red().bold(), e);
        process::exit(1);
    });

    let (ks, model, formula_strings) = match format {
        InputFormat::Pnml => {
            eprintln!(
                "{} The PNML parser has not yet been adapted for the new Arenas architecture.",
                "Not Implemented:".red().bold()
            );
            process::exit(1);
        }
        InputFormat::Prism => {
            eprintln!(
                "{} The Prism parser has not yet been adapted for the new Arenas architecture.",
                "Not Implemented:".red().bold()
            );
            process::exit(1);
        }
        InputFormat::Ssmv => {
            let ast = modeling::ssmv_parser::parse_ssmv(&input_content).unwrap_or_else(|e| {
                eprintln!("{} {}", "SSMV Parser Error:".red().bold(), e);
                process::exit(1);
            });

            let mut strings = Vec::new();
            for &spec_id in &ast.specifications {
                let formatted = ast
                    .ctl_arena
                    .format_formula(spec_id, &|expr_id| ast.arena.format_expr(expr_id));
                strings.push(formatted);
            }

            let symbolic_model = modeling::symbolic::build_model(ast);
            let structure = modeling::expansion::expand_to_kripke(&symbolic_model);

            (structure, symbolic_model, strings)
        }
    };

    let num_states = ks.num_states();
    let num_specs = model.specs.len();

    println!(
        "{} Model loaded via {:?} ({} states). {} formulas found.\n",
        "✔".green(),
        format,
        num_states,
        num_specs
    );

    if num_specs == 0 {
        println!("{}", "No CTL formula found to verify. Exiting.".yellow());
        return;
    }

    let results = match algorithm {
        Algorithm::Labelling => algorithms::labelling::verify(&ks, model),
    };
    for (i, result) in results.into_iter().enumerate() {
        let result_text = if result {
            "TRUE".green().bold()
        } else {
            "FALSE".red().bold()
        };

        println!(
            "{:>2}. [{}] {}\n    └─ Result: {}",
            i + 1,
            "CTL".yellow(),
            formula_strings[i].cyan(),
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
            let result = model.format();

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
