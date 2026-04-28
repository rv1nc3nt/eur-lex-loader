use euro_lex_loader::loader::load_regulation;

fn main() -> Result<(), euro_lex_loader::error::Error> {
    let data_dir = std::path::Path::new("data/EU_AI_ACT");
    let reg = load_regulation(data_dir)?;
    println!("{}", serde_json::to_string_pretty(&reg).expect("serialization failed"));
    Ok(())
}
