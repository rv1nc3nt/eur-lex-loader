mod error;
mod loader;
mod model;
mod parser;
mod text;

use model::ChapterContents;

fn main() -> Result<(), error::Error> {
    let data_dir = std::path::Path::new("data/EU_AI_ACT");
    let reg = loader::load_regulation(data_dir)?;

    println!("=== TITLE ===");
    println!("{}", reg.title);

    println!("\n=== PREAMBLE ===");
    println!("Visas: {} | Recitals: {}", reg.preamble.visas.len(), reg.preamble.recitals.len());
    if let Some(first) = reg.preamble.recitals.first() {
        let preview = &first.text[..first.text.len().min(200)];
        println!("Recital {}: {}...", first.number, preview);
    }

    println!("\n=== ENACTING TERMS ===");
    let mut total_articles = 0usize;
    for chapter in &reg.enacting_terms.chapters {
        match &chapter.contents {
            ChapterContents::Articles(articles) => {
                println!("{} — {} articles", chapter.title, articles.len());
                total_articles += articles.len();
            }
            ChapterContents::Sections(sections) => {
                println!("{} — {} sections", chapter.title, sections.len());
                for section in sections {
                    println!("  {} — {} articles", section.title, section.articles.len());
                    total_articles += section.articles.len();
                }
            }
        }
    }
    println!("Total articles: {}", total_articles);

    println!("\n=== SAMPLE: ARTICLE 1 ===");
    if let Some(chapter) = reg.enacting_terms.chapters.first() {
        let articles = match &chapter.contents {
            ChapterContents::Articles(a) => a.as_slice(),
            ChapterContents::Sections(s) => s.first().map(|s| s.articles.as_slice()).unwrap_or(&[]),
        };
        if let Some(art) = articles.first() {
            println!("Number: {}", art.number);
            if let Some(t) = &art.title {
                println!("Title: {}", t);
            }
            for para in &art.paragraphs {
                if let Some(n) = &para.number {
                    println!("  Para {}:", n);
                }
                for alinea in &para.alineas {
                    println!("    {}", &alinea[..alinea.len().min(120)]);
                }
            }
        }
    }

    println!("\n=== ANNEXES ===");
    for annex in &reg.annexes {
        println!(
            "{}: {} — {} top-level blocks",
            annex.number,
            annex.subtitle.as_deref().unwrap_or("(no subtitle)"),
            annex.content_blocks.len()
        );
    }

    Ok(())
}
