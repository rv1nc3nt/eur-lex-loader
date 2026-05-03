/// Integration tests against the Consumer Rights Directive Formex files in
/// `data/32011L0083` (consolidated version, CELEX 32011L0083).
///
/// Validates that a consolidated directive — a CONS.ACT document with no
/// chapter sub-sections and inline CONS.ANNEX elements — is parsed correctly.
use std::path::Path;

use eur_lex_loader::loader::load_act;
use eur_lex_loader::model::{Act, AnnexContent, ChapterContents, Subparagraph};

#[test]
fn consumer_directive_structure() {
    let loaded = load_act(Path::new("data/32011L0083"))
        .expect("failed to load Consumer Rights Directive");
    let Act::Consolidated(act) = loaded else {
        panic!("Consumer Rights Directive should be a Consolidated act")
    };

    // Title must identify the directive number.
    assert!(
        act.title.contains("2011/83"),
        "title did not contain '2011/83': {}",
        act.title
    );

    // 6 chapters, all with direct articles (no section sub-divisions).
    assert_eq!(act.enacting_terms.chapters.len(), 6, "unexpected chapter count");
    for ch in &act.enacting_terms.chapters {
        assert!(
            matches!(&ch.contents, ChapterContents::Articles(_)),
            "chapter '{}' should have direct articles, not sections",
            ch.title
        );
    }

    // Total articles: 36.
    let total_articles: usize = act.enacting_terms.chapters.iter().map(|c| match &c.contents {
        ChapterContents::Articles(arts) => arts.len(),
        ChapterContents::Sections(secs) => secs.iter().map(|s| s.articles.len()).sum(),
    }).sum();
    assert_eq!(total_articles, 36, "unexpected total article count");

    // 2 inline annexes.
    assert_eq!(act.annexes.len(), 2, "unexpected annex count");
    assert!(act.annexes[0].number.contains("ANNEX I"),
        "expected ANNEX I at index 0, got: {}", act.annexes[0].number);
    assert!(act.annexes[1].number.contains("ANNEX II"),
        "expected ANNEX II at index 1, got: {}", act.annexes[1].number);

    // Annex I has GR.SEQ sub-divisions → Sections.
    assert!(
        matches!(&act.annexes[0].content, AnnexContent::Sections(_)),
        "ANNEX I should be Sections (GR.SEQ)"
    );
    // Annex II has only a GR.TBL (table), no GR.SEQ → Paragraphs.
    assert!(
        matches!(&act.annexes[1].content, AnnexContent::Paragraphs(_)),
        "ANNEX II should be Paragraphs (no GR.SEQ)"
    );

    // Annex II contains the GR.TBL correlation table → at least one Table subparagraph.
    if let AnnexContent::Paragraphs(paras) = &act.annexes[1].content {
        let has_table = paras
            .iter()
            .any(|p| p.alineas.iter().any(|a| matches!(a, Subparagraph::Table(_))));
        assert!(has_table, "ANNEX II should contain at least one Table subparagraph");
    }
}
