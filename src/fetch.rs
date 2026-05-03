use std::path::PathBuf;

use clap::Parser;
use eur_lex_loader::loader::load_act;

/// Fetch a Formex XML publication from EUR-Lex Cellar by CELEX number,
/// extract it to a local directory, and print the act title for verification.
#[derive(Parser)]
#[command(version, about, arg_required_else_help = true)]
struct Cli {
    /// CELEX number of the act to fetch (e.g. 32024R1689).
    celex: String,

    /// Directory where the Formex files will be extracted.
    dir: PathBuf,

    /// Language code (ISO 639-2/B, e.g. eng, fra, deu). Defaults to English.
    #[arg(short, long, default_value = "eng")]
    lang: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    std::fs::create_dir_all(&cli.dir)?;

    eprintln!("Fetching {} ({})...", cli.celex, cli.lang);

    let url = format!("http://publications.europa.eu/resource/celex/{}", cli.celex);
    let bytes = reqwest::blocking::Client::new()
        .get(&url)
        .header("Accept", "application/zip;mtype=fmx4")
        .header("Accept-Language", &cli.lang)
        .send()?
        .error_for_status()?
        .bytes()?;

    zip::ZipArchive::new(std::io::Cursor::new(bytes))?.extract(&cli.dir)?;
    eprintln!("Extracted to {}", cli.dir.display());

    let act = load_act(&cli.dir)?;
    println!("{}", act.title());

    Ok(())
}
