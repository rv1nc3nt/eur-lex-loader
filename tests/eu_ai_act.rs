/// Integration tests against the real EU AI Act Formex files in `data/32024R1689`.
///
/// These tests validate the structural counts established during development
/// and act as a regression guard against parser changes.
use std::path::Path;

use eur_lex_loader::loader::load_act;
use eur_lex_loader::model::{Act, AnnexContent, ChapterContents, CitedActType, Citation, Item, ItemContent, ListBlock, OjRef, Subparagraph};

#[test]
fn eu_ai_act_structure() {
    let act = load_act(Path::new("data/32024R1689"))
        .expect("failed to load EU AI Act from data/32024R1689");
    let Act::Regular(reg) = act else { panic!("EU AI Act should be a Regular act") };

    // Title must identify the act number.
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
            assert!(matches!(&lb.items[0], Item { number: 1, content: ItemContent::Text(_) }),
                "first item should be at position 1");
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
    assert!(matches!(&para1.alineas[1], Subparagraph::Text(_)),
        "Article 5 para 1 alineas[1] should be trailing plain Text");
    match &para1.alineas[0] {
        Subparagraph::List(lb) => {
            assert_eq!(lb.items.len(), 8, "Article 5 list should have 8 items");
            assert!(matches!(&lb.items[0], Item { number: 1, content: ItemContent::Text(_) }));
            // Item (c) has 2 sub-items.
            match &lb.items[2] {
                Item { number: 3, content: ItemContent::List(inner) } => {
                    assert_eq!(inner.items.len(), 2, "Article 5 item (c) should have 2 sub-items");
                }
                _ => panic!("Article 5 items[2] should be a nested List for (c)"),
            }
            // Item (h) has 3 sub-items.
            match &lb.items[7] {
                Item { number: 8, content: ItemContent::List(inner) } => {
                    assert_eq!(inner.items.len(), 3, "Article 5 item (h) should have 3 sub-items");
                }
                _ => panic!("Article 5 items[7] should be a nested List for (h)"),
            }
        }
        _ => panic!("Article 5 para 1 alineas[0] should be a List"),
    }

    // Annex III (index 2): list wrapped in a <P> → Paragraphs mode, single List block with 8 items.
    let annex_iii = &reg.annexes[2];
    assert!(annex_iii.number.contains("III"), "expected ANNEX III at index 2");
    let iii_paragraphs = match &annex_iii.content {
        AnnexContent::Paragraphs(p) => p,
        AnnexContent::Sections(_) => panic!("expected Paragraphs for Annex III"),
    };
    let iii_list = iii_paragraphs.iter()
        .flat_map(|p| p.alineas.iter())
        .find_map(|b| if let Subparagraph::List(lb) = b { Some(lb) } else { None })
        .expect("Annex III should contain a List block");
    assert_eq!(iii_list.items.len(), 8, "Annex III should have 8 high-risk category items");
    // Item 0 (Biometrics) has 3 alpha sub-items.
    match &iii_list.items[0] {
        Item { content: ItemContent::List(inner), .. } =>
            assert_eq!(inner.items.len(), 3, "Annex III item 1 should have 3 sub-items"),
        _ => panic!("expected nested List for Annex III item 1"),
    }
    // Item 1 (Critical infrastructure) has no sub-items.
    assert!(matches!(&iii_list.items[1], Item { content: ItemContent::Text(_), .. }),
        "Annex III item 2 should be a plain Text (no sub-items)");

    // Annex IV (index 3): NP-wrapped list items → Paragraphs mode, List block with no empty text.
    let annex_iv = &reg.annexes[3];
    assert!(annex_iv.number.contains("IV"), "expected ANNEX IV at index 3");
    let iv_paragraphs = match &annex_iv.content {
        AnnexContent::Paragraphs(p) => p,
        AnnexContent::Sections(_) => panic!("expected Paragraphs for Annex IV"),
    };
    let iv_list = iv_paragraphs.iter()
        .flat_map(|p| p.alineas.iter())
        .find_map(|b| if let Subparagraph::List(lb) = b { Some(lb) } else { None })
        .expect("Annex IV should contain a List block");
    let empty_items: Vec<_> = iv_list.items.iter()
        .filter(|item| matches!(item, Item { content: ItemContent::Text(t), .. } if t.is_empty()))
        .collect();
    assert!(empty_items.is_empty(), "Annex IV has {} list item(s) with empty text", empty_items.len());

    // Annexes: 13 files, all identified as ANNEX something.
    assert_eq!(reg.annexes.len(), 13, "unexpected annex count");
    for annex in &reg.annexes {
        assert!(
            annex.number.contains("ANNEX"),
            "annex number did not contain 'ANNEX': {}",
            annex.number
        );
    }

    // Definitions: Article 3 has 68 items, all extracted into the map.
    assert_eq!(reg.definitions.len(), 68, "EU AI Act should have 68 definitions");
    assert!(
        reg.definitions.contains_key("AI system"),
        "definitions should contain 'AI system'"
    );
}

#[test]
fn eu_ai_act_recital_citations() {
    let act = load_act(Path::new("data/32024R1689"))
        .expect("failed to load EU AI Act from data/32024R1689");
    let Act::Regular(reg) = act else { panic!("EU AI Act should be a Regular act") };

    let recitals = &reg.preamble.recitals;

    // Recital (10): four NOTE-backed citations — GDPR, Regulation 2018/1725,
    // Law Enforcement Directive, and Directive 2002/58/EC (ePrivacy).
    // Source: L_202401689EN.000101.fmx.xml, CONSID (10).
    let r10 = &recitals[9].citations;
    assert!(
        r10.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EU".into()), number: "2016/679".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "119".into(), date: "20160504".into(), page: 1 }) }),
        "recital (10): missing Regulation (EU) 2016/679 (GDPR)"
    );
    assert!(
        r10.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EU".into()), number: "2018/1725".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "295".into(), date: "20181121".into(), page: 39 }) }),
        "recital (10): missing Regulation (EU) 2018/1725"
    );
    assert!(
        r10.contains(&Citation { act_type: CitedActType::Directive, regime: Some("EU".into()), number: "2016/680".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "119".into(), date: "20160504".into(), page: 89 }) }),
        "recital (10): missing Directive (EU) 2016/680"
    );
    assert!(
        r10.contains(&Citation { act_type: CitedActType::Directive, regime: Some("EC".into()), number: "2002/58".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "201".into(), date: "20020731".into(), page: 37 }) }),
        "recital (10): missing Directive 2002/58/EC (ePrivacy)"
    );

    // Recital (11): DSA cited with OJ ref via NOTE.
    // Source: L_202401689EN.000101.fmx.xml, CONSID (11).
    let r11 = &recitals[10].citations;
    assert!(
        r11.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EU".into()), number: "2022/2065".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "277".into(), date: "20221027".into(), page: 1 }) }),
        "recital (11): missing Regulation (EU) 2022/2065 (DSA)"
    );

    // Recital (14): three inline citations only — no NOTEs in this recital.
    // Source: L_202401689EN.000101.fmx.xml, CONSID (14).
    let r14 = &recitals[13].citations;
    assert!(
        r14.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EU".into()),
            number: "2016/679".into(), oj_ref: None }),
        "recital (14): missing inline Regulation (EU) 2016/679"
    );
    assert!(
        r14.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EU".into()),
            number: "2018/1725".into(), oj_ref: None }),
        "recital (14): missing inline Regulation (EU) 2018/1725"
    );
    assert!(
        r14.contains(&Citation { act_type: CitedActType::Directive, regime: Some("EU".into()),
            number: "2016/680".into(), oj_ref: None }),
        "recital (14): missing inline Directive (EU) 2016/680"
    );
    // Inline-only citations must not carry an OJ ref.
    for c in r14 {
        assert!(c.oj_ref.is_none(), "recital (14): all citations should be inline (no OJ ref), found {:?}", c);
    }
}
