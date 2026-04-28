/// Integration tests against the real EU AI Act Formex files in `data/EU_AI_ACT`.
///
/// These tests validate the structural counts established during development
/// and act as a regression guard against parser changes.
use std::path::Path;

use euro_lex_loader::loader::load_regulation;
use euro_lex_loader::model::ChapterContents;

#[test]
fn eu_ai_act_structure() {
    let reg = load_regulation(Path::new("data/EU_AI_ACT"))
        .expect("failed to load EU AI Act from data/EU_AI_ACT");

    // Title must identify the regulation number.
    assert!(
        reg.title.contains("2024/1689"),
        "title did not contain '2024/1689': {}",
        reg.title
    );

    // Preamble: 7 legal bases, 180 recitals.
    assert_eq!(reg.preamble.visas.len(), 7, "unexpected visa count");
    assert_eq!(reg.preamble.recitals.len(), 180, "unexpected recital count");
    assert_eq!(
        reg.preamble.recitals[0].number, "(1)",
        "first recital number mismatch"
    );

    // Enacting terms: 13 chapters, 113 articles total.
    assert_eq!(reg.enacting_terms.chapters.len(), 13, "unexpected chapter count");

    let total_articles: usize = reg.enacting_terms.chapters.iter().map(|c| match &c.contents {
        ChapterContents::Articles(arts) => arts.len(),
        ChapterContents::Sections(secs) => secs.iter().map(|s| s.articles.len()).sum(),
    }).sum();
    assert_eq!(total_articles, 113, "unexpected total article count");

    // Annexes: 13 files, all identified as ANNEX something.
    assert_eq!(reg.annexes.len(), 13, "unexpected annex count");
    for annex in &reg.annexes {
        assert!(
            annex.number.contains("ANNEX"),
            "annex number did not contain 'ANNEX': {}",
            annex.number
        );
    }
}
