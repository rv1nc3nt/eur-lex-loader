/// Integration tests against the REACH Regulation Formex files in
/// `data/REACH_reg` (consolidated version, CELEX 32006R1907).
///
/// These tests validate that consolidated acts — which use `<CONS.ACT>` as
/// the root element and embed annexes as `<CONS.ANNEX>` elements — are parsed
/// correctly.
use std::path::Path;

use eur_lex_loader::loader::load_act;
use eur_lex_loader::model::{Act, AnnexContent, ChapterContents, Subparagraph};

#[test]
fn reach_regulation_structure() {
    let loaded = load_act(Path::new("data/REACH_reg"))
        .expect("failed to load REACH regulation from data/REACH_reg");
    let Act::Consolidated(reg) = loaded else {
        panic!("REACH regulation should be a Consolidated act")
    };

    // Title must identify the regulation number.
    assert!(
        reg.title.contains("1907/2006"),
        "title did not contain '1907/2006': {}",
        reg.title
    );

    // 15 top-level titles.
    assert_eq!(reg.enacting_terms.chapters.len(), 15, "unexpected chapter count");

    // Total articles: 141.
    let total_articles: usize = reg.enacting_terms.chapters.iter().map(|c| match &c.contents {
        ChapterContents::Articles(arts) => arts.len(),
        ChapterContents::Sections(secs) => secs.iter().map(|s| s.articles.len()).sum(),
    }).sum();
    assert_eq!(total_articles, 141, "unexpected total article count");

    // 18 inline CONS.ANNEX elements (including the LIST OF ANNEXES meta-entry).
    assert_eq!(reg.annexes.len(), 18, "unexpected annex count");

    // Annexes I through XVII are present.
    assert!(reg.annexes[1].number.contains("ANNEX I"),
        "expected ANNEX I at index 1, got: {}", reg.annexes[1].number);
    for annex in &reg.annexes[1..] {
        assert!(
            annex.number.contains("ANNEX"),
            "annex number did not contain 'ANNEX': {}",
            annex.number
        );
    }

    // ANNEX IV is a flat table (TBL directly inside CONTENTS) — verify a Table is parsed.
    let annex_iv = reg.annexes.iter().find(|a| a.number.contains("ANNEX IV"))
        .expect("ANNEX IV not found");
    let has_table = match &annex_iv.content {
        AnnexContent::Paragraphs(paras) => paras.iter()
            .any(|p| p.alineas.iter().any(|a| matches!(a, Subparagraph::Table(_)))),
        AnnexContent::Sections(secs) => secs.iter()
            .any(|s| s.alineas.iter().any(|a| matches!(a, Subparagraph::Table(_)))),
    };
    assert!(has_table, "ANNEX IV should contain at least one Table subparagraph");
}
