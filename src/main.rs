use euro_lex_loader::loader::load_regulation;

/// Loads a Formex regulation directory and prints the result as pretty-printed JSON.
///
/// Usage: `euro-lex-loader [DATA_DIR]`
///
/// `DATA_DIR` defaults to `data/EU_AI_ACT` when omitted.
fn main() -> Result<(), euro_lex_loader::error::Error> {
    let arg = std::env::args().nth(1);
    let data_dir = std::path::Path::new(arg.as_deref().unwrap_or("data/EU_AI_ACT"));
    let reg = load_regulation(data_dir)?;
    println!("{}", serde_json::to_string_pretty(&reg).expect("serialization failed"));
    Ok(())
}
