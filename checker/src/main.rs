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
use oxidd::{Manager, ManagerRef};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process;
use std::time::{Duration, Instant};

/// Structure to hold benchmark data for CSV export
struct BenchmarkRecord {
    model_name: String,
    algorithm: String,
    parse_time_ms: u128,
    compilation_time_ms: u128,
    verification_time_ms: u128,
    total_time_ms: u128,
    static_nodes: usize,
    verification_nodes: usize,
    explicit_states: usize,
}

/// Simple milestone printer (Time and Current Memory)
fn print_milestone(name: &str, elapsed: Duration) {
    let current_mem_mb = memory_stats()
        .map(|usage| usage.physical_mem as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0);

    println!(
        "{} {:<25} | {:>10.2?} | Cur Mem: {:>7.2} MB",
        "⏱".cyan(),
        name.bold(),
        elapsed,
        current_mem_mb
    );
}

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
    let mut record = BenchmarkRecord {
        model_name: model_path.clone(),
        algorithm: format!("{:?}", algorithm),
        parse_time_ms: 0,
        compilation_time_ms: 0,
        verification_time_ms: 0,
        total_time_ms: 0,
        static_nodes: 0,
        verification_nodes: 0,
        explicit_states: 0,
    };

    // --- Phase 1: Parsing and IR Generation ---
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
                "{} Format {:?} not supported.",
                "Error:".red().bold(),
                format
            );
            process::exit(1);
        }
    };
    record.parse_time_ms = phase_start.elapsed().as_millis();
    print_milestone("Parse & IR Generation", phase_start.elapsed());

    // --- Phase 2: CTL Verification ---
    let verify_phase_start = Instant::now();
    let results = match algorithm {
        Algorithm::Bdd => {
            println!("{} Using Symbolic BDD algorithm (OxiDD)...", "ℹ".blue());

            let compile_start = Instant::now();
            let symbolic_ctx = modeling::bdd_compiler::compile_model_to_bdd(&symbolic_model);

            // Collect BDD node count from OxiDD Manager
            record.static_nodes = symbolic_ctx
                .manager
                .with_manager_shared(|m| m.num_inner_nodes());
            record.compilation_time_ms = compile_start.elapsed().as_millis();
            print_milestone("BDD Compilation", compile_start.elapsed());

            let results = algorithms::bdd_fixpoint::verify(&symbolic_ctx, symbolic_model);
            record.verification_nodes = symbolic_ctx
                .manager
                .with_manager_shared(|m| m.num_inner_nodes());
            results
        }

        Algorithm::Labelling | Algorithm::LabellingScc => {
            let expansion_start = Instant::now();
            let ks = modeling::expansion::expand_to_kripke(&symbolic_model);

            record.explicit_states = ks.num_states();
            record.compilation_time_ms = expansion_start.elapsed().as_millis();
            print_milestone("State Space Expansion", expansion_start.elapsed());

            match algorithm {
                Algorithm::Labelling => algorithms::labelling::verify(&ks, symbolic_model),
                Algorithm::LabellingScc => algorithms::labelling_scc::verify(&ks, symbolic_model),
                _ => unreachable!(),
            }
        }
    };

    record.verification_time_ms =
        verify_phase_start.elapsed().as_millis() - record.compilation_time_ms;
    record.total_time_ms = total_start.elapsed().as_millis();

    print_milestone("Total Verification Phase", verify_phase_start.elapsed());
    println!();

    // --- Phase 3: Results Output ---
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
    print_milestone("Full Program Execution", total_start.elapsed());

    save_to_csv(record);
}

/// Appends the benchmark results to a CSV file
fn save_to_csv(r: BenchmarkRecord) {
    let file_name = "benchmarks.csv";
    let file_exists = std::path::Path::new(file_name).exists();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_name)
        .unwrap();

    if !file_exists {
        writeln!(
            file,
            "model,algorithm,parse_ms,compile_ms,verify_ms,total_ms,static_nodes,verification_nodes,states"
        )
        .unwrap();
    }

    writeln!(
        file,
        "{},{},{},{},{},{},{},{},{}",
        r.model_name,
        r.algorithm,
        r.parse_time_ms,
        r.compilation_time_ms,
        r.verification_time_ms,
        r.total_time_ms,
        r.static_nodes,
        r.verification_nodes,
        r.explicit_states
    )
    .unwrap();

    println!("{} Results appended to {}", "✔".green(), file_name);
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
