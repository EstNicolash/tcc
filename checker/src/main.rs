mod algorithms;
mod cli;
mod core;
mod io;
mod modeling;
mod specs;

use clap::Parser;
use cli::{Algorithm, Cli, Commands, InputFormat};
use colored::*;
use memory_stats::memory_stats;
use std::fs;
use std::process;
use std::time::Instant;

// --- Helper function for benchmarking ---
fn print_milestone(name: &str, start_time: Instant) {
    let elapsed = start_time.elapsed();

    // Fetch current memory usage
    let memory_mb = if let Some(usage) = memory_stats() {
        usage.physical_mem as f64 / (1024.0 * 1024.0) // Convert bytes to MB
    } else {
        0.0
    };

    println!(
        "{} [{}] Time: {:.2?}, Memory: {:.2} MB",
        "⏱".cyan(),
        name.bold(),
        elapsed,
        memory_mb
    );
}
// -----------------------------------------

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

    let total_start = Instant::now();

    let input_content = fs::read_to_string(&model_path).unwrap_or_else(|e| {
        eprintln!("{} {}", "Error reading file:".red().bold(), e);
        process::exit(1);
    });

    let phase_start = Instant::now();

    let (symbolic_model, formula_strings) = match format {
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

            let model = modeling::symbolic::build_model(ast);
            (model, strings)
        }
        _ => {
            eprintln!(
                "{} Format {:?} not supported yet in the new architecture.",
                "Error:".red().bold(),
                format
            );
            process::exit(1);
        }
    };

    print_milestone("Parse & IR Generation", phase_start);

    let verify_start = Instant::now();

    let results = match algorithm {
        Algorithm::Bdd => {
            println!("{} Using Symbolic BDD algorithm (OxiDD)...", "ℹ".blue());

            let symbolic_ctx = modeling::bdd_compiler::compile_model_to_bdd(&symbolic_model);
            print_milestone("BDD Compilation (I & Delta)", verify_start);

            algorithms::bdd_fixpoint::verify(&symbolic_ctx, symbolic_model)
        }

        Algorithm::Labelling | Algorithm::LabellingScc => {
            println!(
                "{} Expanding state space for explicit algorithm...",
                "ℹ".blue()
            );
            let expansion_start = Instant::now();

            let ks = modeling::expansion::expand_to_kripke(&symbolic_model);
            print_milestone("State Space Expansion", expansion_start);

            println!(
                "{} Model expanded to {} states.",
                "✔".green(),
                ks.num_states()
            );

            match algorithm {
                Algorithm::Labelling => {
                    println!("{} Using Naive Labelling algorithm...", "ℹ".blue());
                    algorithms::labelling::verify(&ks, symbolic_model)
                }
                Algorithm::LabellingScc => {
                    println!("{} Using SCC-based Labelling (Tarjan)...", "ℹ".blue());
                    algorithms::labelling_scc::verify(&ks, symbolic_model)
                }
                _ => unreachable!(),
            }
        }
    };

    print_milestone("CTL Verification Phase", verify_start);
    println!();

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

    println!();
    print_milestone("Total Execution", total_start);
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
