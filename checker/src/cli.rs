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
        spec_path: Option<String>,

        #[arg(short, long, value_parser = parse_order)]
        order: Option<OrderInput>,

        /// Format of the model input
        #[arg(short, long, value_enum, default_value_t = InputFormat::Ssmv)]
        format: InputFormat,

        /// Algorithm to use for verification
        #[arg(short, long, value_enum, default_value_t = Algorithm::Bdd)]
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

#[derive(Clone, Debug)]
pub enum OrderInput {
    Default,
    File(String),
    Random(u64),
    Force(usize),
}

fn parse_order(s: &str) -> Result<OrderInput, String> {
    if s.to_lowercase() == "default" {
        return Ok(OrderInput::Default);
    }

    if let Some(seed_str) = s.strip_prefix("random:") {
        let seed = seed_str
            .parse::<u64>()
            .map_err(|_| format!("Invalid seed '{}'. u64 expected.", seed_str))?;
        return Ok(OrderInput::Random(seed));
    }

    if let Some(iter_str) = s.strip_prefix("force:") {
        let iterations = iter_str
            .parse::<usize>()
            .map_err(|_| format!("Invalid iteration number '{}'. usize expected.", iter_str))?;
        return Ok(OrderInput::Force(iterations));
    }

    Ok(OrderInput::File(s.to_string()))
}
