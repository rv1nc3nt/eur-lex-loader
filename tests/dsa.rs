/// Integration tests against the real Digital Services Act Formex files in `data/32022R2065`.
///
/// These tests validate the structural counts established during development
/// and act as a regression guard against parser changes.
use std::path::Path;

use eur_lex_loader::loader::load_act;
use eur_lex_loader::model::{Act, ChapterContents, Item, ItemContent, Subparagraph};

#[test]
fn dsa_structure() {
    let act = load_act(Path::new("data/32022R2065")).expect("failed to load DSA from data/32022R2065");
    let Act::Regular(reg) = act else { panic!("DSA should be a Regular act") };

    // Title must identify the act number.
    assert!(
        reg.title.contains("2022/2065"),
        "title did not contain '2022/2065': {}",
        reg.title
    );

    // Preamble: 6 legal bases, 156 recitals.
    assert_eq!(reg.preamble.visas.len(), 6, "unexpected visa count");
    assert_eq!(reg.preamble.recitals.len(), 156, "unexpected recital count");
    assert_eq!(
        reg.preamble.recitals[0].number, "(1)",
        "first recital number mismatch"
    );

    // Enacting terms: 5 chapters, 93 articles total.
    assert_eq!(
        reg.enacting_terms.chapters.len(),
        5,
        "unexpected chapter count"
    );

    let total_articles: usize = reg
        .enacting_terms
        .chapters
        .iter()
        .map(|c| match &c.contents {
            ChapterContents::Articles(arts) => arts.len(),
            ChapterContents::Sections(secs) => secs.iter().map(|s| s.articles.len()).sum(),
        })
        .sum();
    assert_eq!(total_articles, 93, "unexpected total article count");

    // No annexes in the DSA.
    assert_eq!(reg.annexes.len(), 0, "DSA should have no annexes");

    // Chapter I (idx 0): 3 direct articles.
    let ch1_arts = match &reg.enacting_terms.chapters[0].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter I should have direct articles"),
    };
    assert_eq!(ch1_arts.len(), 3, "Chapter I should have 3 articles");

    // Article 1 ("Subject matter"): 2 paragraphs.
    let art1 = &ch1_arts[0];
    assert_eq!(
        art1.number, "Article 1",
        "unexpected article at index 0 of Chapter I"
    );
    assert_eq!(art1.title.as_deref(), Some("Subject matter"));
    assert_eq!(
        art1.paragraphs.len(),
        2,
        "Article 1 should have 2 paragraphs"
    );

    // Para 1 (number "1."): 1 block — plain Paragraph.
    let p1 = &art1.paragraphs[0];
    assert_eq!(p1.number.as_deref(), Some("1."));
    assert_eq!(
        p1.alineas.len(),
        1,
        "Article 1 para 1 should have 1 alinea block"
    );
    assert!(
        matches!(&p1.alineas[0], Subparagraph::Text(_)),
        "Article 1 para 1 alineas[0] should be a plain Text"
    );

    // Para 2 (number "2."): intro + 3 list items (a)(b)(c) grouped into one List.
    let p2 = &art1.paragraphs[1];
    assert_eq!(p2.number.as_deref(), Some("2."));
    assert_eq!(
        p2.alineas.len(),
        1,
        "Article 1 para 2 should be a single List block"
    );
    match &p2.alineas[0] {
        Subparagraph::List(lb) => {
            assert_eq!(lb.items.len(), 3, "para 2 list should have 3 items");
            assert!(matches!(&lb.items[0], Item { number: 1, content: ItemContent::Text(_) }));
            assert!(matches!(&lb.items[1], Item { number: 2, content: ItemContent::Text(_) }));
            assert!(matches!(&lb.items[2], Item { number: 3, content: ItemContent::Text(_) }));
        }
        _ => panic!("Article 1 para 2 alineas[0] should be a List"),
    }

    // Article 3 ("Definitions", idx 2): bare <ALINEA> (no <PARAG> wrapper)
    // containing <P> + <LIST> with 24 items → 1 unnamed paragraph, 25 alinea blocks.
    let art3 = &ch1_arts[2];
    assert_eq!(
        art3.number, "Article 3",
        "unexpected article at index 2 of Chapter I"
    );
    assert_eq!(art3.paragraphs.len(), 1);
    assert!(
        art3.paragraphs[0].number.is_none(),
        "Article 3 bare-alinea paragraph should have no number"
    );
    assert_eq!(
        art3.paragraphs[0].alineas.len(),
        1,
        "Article 3 should be a single List block (intro + 24 items)"
    );
    match &art3.paragraphs[0].alineas[0] {
        Subparagraph::List(lb) => {
            assert_eq!(lb.items.len(), 24, "Article 3 list should have 24 definition items");
        }
        _ => panic!("Article 3 alineas[0] should be a List"),
    }

    // Chapter II (idx 1): 7 direct articles.
    let ch2_arts = match &reg.enacting_terms.chapters[1].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter II should have direct articles"),
    };
    assert_eq!(ch2_arts.len(), 7, "Chapter II should have 7 articles");

    // Chapter III (idx 2): 6 sections, 38 articles total (5+3+10+4+11+5).
    let ch3_secs = match &reg.enacting_terms.chapters[2].contents {
        ChapterContents::Sections(secs) => secs,
        _ => panic!("Chapter III should have sections"),
    };
    assert_eq!(ch3_secs.len(), 6, "Chapter III should have 6 sections");
    let ch3_total: usize = ch3_secs.iter().map(|s| s.articles.len()).sum();
    assert_eq!(ch3_total, 38, "Chapter III should have 38 total articles");
    assert_eq!(
        ch3_secs[0].articles.len(),
        5,
        "Chapter III section 0 should have 5 articles"
    );
    assert_eq!(
        ch3_secs[2].articles.len(),
        10,
        "Chapter III section 2 should have 10 articles"
    );

    // Chapter IV (idx 3): 6 sections, 40 articles total (7+5+3+20+3+2).
    let ch4_secs = match &reg.enacting_terms.chapters[3].contents {
        ChapterContents::Sections(secs) => secs,
        _ => panic!("Chapter IV should have sections"),
    };
    assert_eq!(ch4_secs.len(), 6, "Chapter IV should have 6 sections");
    let ch4_total: usize = ch4_secs.iter().map(|s| s.articles.len()).sum();
    assert_eq!(ch4_total, 40, "Chapter IV should have 40 total articles");
    assert_eq!(
        ch4_secs[3].articles.len(),
        20,
        "Chapter IV section 3 should have 20 articles"
    );

    // Chapter V (idx 4): 5 direct articles.
    let ch5_arts = match &reg.enacting_terms.chapters[4].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter V should have direct articles"),
    };
    assert_eq!(ch5_arts.len(), 5, "Chapter V should have 5 articles");

    // Definitions: Article 3 has 27 items.
    assert_eq!(reg.definitions.len(), 27, "DSA should have 27 definitions");
    assert!(
        reg.definitions.contains_key("intermediary service"),
        "definitions should contain 'intermediary service'"
    );
}
