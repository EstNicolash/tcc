use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone, Debug)]
pub enum InputFormat {
    Pnml,
    Prism,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum Algorithm {
    Labelling,
    //Coming soon...
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Rust Model Checker")]
pub struct Args {
    /// Path to the model file (.fsm or .pnml/.prism)
    pub model_path: String,

    /// Path to the specification file (.spec)
    pub spec_path: String,

    /// Format of the metadata input
    #[arg(short, long, value_enum, default_value_t = InputFormat::Pnml)]
    pub format: InputFormat,

    /// Algorithm to use for verification
    #[arg(short, long, value_enum, default_value_t = Algorithm::Labelling)]
    pub algorithm: Algorithm,

    /// Verbose: shows detailed processing information
    #[arg(short, long)]
    pub verbose: bool,
}
