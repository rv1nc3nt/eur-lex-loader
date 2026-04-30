/// Integration tests against the real EU AI Act Formex files in `data/EU_AI_ACT`.
///
/// These tests validate the structural counts established during development
/// and act as a regression guard against parser changes.
use std::path::Path;

use eur_lex_loader::loader::load_regulation;
use eur_lex_loader::model::{ChapterContents, ContentBlock, ListBlock, Subparagraph};

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

    // Article 3 (definitions): intro + 68 items grouped into a single List block.
    let ch1_arts = match &reg.enacting_terms.chapters[0].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter I should have direct articles"),
    };
    let art3 = &ch1_arts[2];
    assert_eq!(art3.number, "Article 3", "unexpected article at index 2 of Chapter I");
    assert_eq!(art3.paragraphs.len(), 1);
    assert_eq!(
        art3.paragraphs[0].alineas.len(), 1,
        "Article 3 should be a single List block"
    );
    match &art3.paragraphs[0].alineas[0] {
        Subparagraph::List(lb) => {
            assert_eq!(lb.items.len(), 68, "Article 3 list should have 68 definition items");
            assert!(matches!(&lb.items[0], Subparagraph::Text { number: Some(n), .. } if n == "(1)"),
                "first item should be numbered (1)");
        }
        _ => panic!("Article 3 alineas[0] should be a List"),
    }

    // Article 5, paragraph 1: intro+list grouped into one List block, plus
    // a trailing plain Text. Items (c) and (h) carry nested Lists.
    let ch2_arts = match &reg.enacting_terms.chapters[1].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter II should have direct articles"),
    };
    let ch1_art5 = &ch2_arts[0];
    assert_eq!(ch1_art5.number, "Article 5", "unexpected article at index 0 of Chapter II");
    let para1 = &ch1_art5.paragraphs[0];
    assert_eq!(para1.number.as_deref(), Some("1."));
    // 1 List block + 1 trailing plain Text = 2
    assert_eq!(para1.alineas.len(), 2, "Article 5 para 1 should have 2 alinea blocks");
    assert!(matches!(&para1.alineas[1], Subparagraph::Text { number: None, .. }),
        "Article 5 para 1 alineas[1] should be trailing plain Text");
    match &para1.alineas[0] {
        Subparagraph::List(lb) => {
            assert_eq!(lb.items.len(), 8, "Article 5 list should have 8 items");
            assert!(matches!(&lb.items[0], Subparagraph::Text { number: Some(n), .. } if n == "(a)"));
            // Item (c) has 2 sub-items.
            match &lb.items[2] {
                Subparagraph::List(ListBlock { number, items, .. }) => {
                    assert_eq!(number.as_deref(), Some("(c)"));
                    assert_eq!(items.len(), 2, "Article 5 item (c) should have 2 sub-items");
                }
                _ => panic!("Article 5 items[2] should be a nested List for (c)"),
            }
            // Item (h) has 3 sub-items.
            match &lb.items[7] {
                Subparagraph::List(ListBlock { number, items, .. }) => {
                    assert_eq!(number.as_deref(), Some("(h)"));
                    assert_eq!(items.len(), 3, "Article 5 item (h) should have 3 sub-items");
                }
                _ => panic!("Article 5 items[7] should be a nested List for (h)"),
            }
        }
        _ => panic!("Article 5 para 1 alineas[0] should be a List"),
    }

    // Annex III (index 2): list wrapped in a <P> must expand to 8 ListItems.
    let annex_iii = &reg.annexes[2];
    assert!(annex_iii.number.contains("III"), "expected ANNEX III at index 2");
    let iii_items: Vec<_> = annex_iii.content_blocks.iter()
        .filter(|b| matches!(b, ContentBlock::ListItem { .. }))
        .collect();
    assert_eq!(iii_items.len(), 8, "Annex III should have 8 high-risk category items");
    // Item 1 (Biometrics) has 3 alpha sub-items; item 2 (Critical infrastructure) has none.
    match &annex_iii.content_blocks[1] {
        ContentBlock::ListItem { sub_items, .. } =>
            assert_eq!(sub_items.len(), 3, "Annex III item 1 should have 3 sub-items"),
        _ => panic!("expected ListItem at index 1 of Annex III"),
    }
    match &annex_iii.content_blocks[2] {
        ContentBlock::ListItem { sub_items, .. } =>
            assert!(sub_items.is_empty(), "Annex III item 2 should have no sub-items"),
        _ => panic!("expected ListItem at index 2 of Annex III"),
    }

    // Annex IV (index 3): list items use <NP> wrappers and must not have empty text.
    let annex_iv = &reg.annexes[3];
    assert!(annex_iv.number.contains("IV"), "expected ANNEX IV at index 3");
    let empty_items: Vec<_> = annex_iv.content_blocks.iter().filter(|b| {
        matches!(b, ContentBlock::ListItem { text, .. } if text.is_empty())
    }).collect();
    assert!(empty_items.is_empty(), "Annex IV has {} ListItem(s) with empty text", empty_items.len());

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
