use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(author, version, about = "Rust Model Checker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Verbose: shows detailed processing information
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Verify {
        /// Path to the model file (.pnml, .prism, .smv)
        model_path: String,

        /// Path to the specification file (.spec)
        spec_path: String,

        /// Format of the model input
        #[arg(short, long, value_enum, default_value_t = InputFormat::Pnml)]
        format: InputFormat,

        /// Algorithm to use for verification
        #[arg(short, long, value_enum, default_value_t = Algorithm::Labelling)]
        algorithm: Algorithm,
    },

    TestParser {
        /// Path to the .smv file to parse
        input_file: String,

        /// Optional: Path to save the parsed output
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Pnml,
    Prism,
    Ssmv,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum Algorithm {
    Labelling,
    LabellingScc,
    Bdd,
}
