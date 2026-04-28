use std::path::PathBuf;

use clap::Parser;
use euro_lex_loader::loader::load_regulation;

/// Load a Formex regulation directory and output it as JSON.
///
/// The directory must contain a `*.doc.fmx.xml` registry file that lists the
/// main act and all annex files. See the EU AI Act example in `data/EU_AI_ACT`.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Path to the Formex regulation directory.
    #[arg(default_value = "data/EU_AI_ACT")]
    dir: PathBuf,

    /// Write JSON output to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Output compact JSON (default: pretty-printed).
    #[arg(short, long)]
    compact: bool,
}

fn main() -> Result<(), euro_lex_loader::error::Error> {
    let cli = Cli::parse();
    let reg = load_regulation(&cli.dir)?;

    let json = if cli.compact {
        serde_json::to_string(&reg)
    } else {
        serde_json::to_string_pretty(&reg)
    }
    .expect("serialization failed");

    match cli.output {
        Some(ref path) => std::fs::write(path, json).map_err(|e| {
            euro_lex_loader::error::Error::Io { path: path.display().to_string(), source: e }
        })?,
        None => println!("{json}"),
    }

    Ok(())
}
