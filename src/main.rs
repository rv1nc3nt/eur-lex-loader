use std::path::PathBuf;

use clap::Parser;
use eur_lex_loader::loader::load_act;
use eur_lex_loader::model::Act;

/// Load a Formex act and output it as JSON.
///
/// Pass a local directory path, or use `--celex` to fetch directly from
/// the EUR-Lex Cellar repository. The directory must contain a `*.doc.fmx.xml`
/// or `*.doc.xml` registry file.
#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
struct Cli {
    /// Path to the Formex act directory (conflicts with --celex).
    dir: Option<PathBuf>,

    /// Fetch an act from EUR-Lex Cellar by CELEX number (e.g. 32022R2065).
    #[arg(short, long, conflicts_with = "dir")]
    celex: Option<String>,

    /// Write JSON output to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,

    /// Output compact JSON (default: pretty-printed).
    #[arg(long)]
    compact: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let reg = match (cli.celex.as_deref(), cli.dir.as_deref()) {
        (Some(celex), _) => fetch_by_celex(celex)?,
        (None, Some(dir)) => load_act(dir)?,
        (None, None) => unreachable!("clap enforces arg_required_else_help"),
    };

    let json = if cli.compact {
        serde_json::to_string(&reg)
    } else {
        serde_json::to_string_pretty(&reg)
    }
    .expect("serialization failed");

    match cli.output {
        Some(ref path) => std::fs::write(path, &json).map_err(|e| {
            eur_lex_loader::error::Error::Io { path: path.display().to_string(), source: e }
        })?,
        None => println!("{json}"),
    }

    Ok(())
}

/// Fetches a Formex publication from the EUR-Lex Cellar API by CELEX number,
/// extracts the ZIP to a temporary directory, and parses it via
/// [`load_act`].
fn fetch_by_celex(celex: &str) -> Result<Act, Box<dyn std::error::Error>> {
    let url = format!("http://publications.europa.eu/resource/celex/{celex}");
    let bytes = reqwest::blocking::Client::new()
        .get(&url)
        .header("Accept", "application/zip;mtype=fmx4")
        .header("Accept-Language", "eng")
        .send()?
        .error_for_status()?
        .bytes()?;

    let tmp = tempfile::tempdir()?;
    zip::ZipArchive::new(std::io::Cursor::new(bytes))?.extract(tmp.path())?;

    // `tmp` must remain in scope until load_act returns: TempDir deletes
    // the directory on drop, so moving it out or shortening its scope would
    // cause load_act to receive a path that no longer exists.
    Ok(load_act(tmp.path())?)
}
