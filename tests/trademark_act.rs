/// Integration tests against the real EU Trade Mark Regulation Formex files
/// in `data/32017R1001`.
///
/// These tests validate the structural counts established during development
/// and act as a regression guard against parser changes.
use std::path::Path;

use eur_lex_loader::loader::load_act;
use eur_lex_loader::model::{Act, AnnexContent, ChapterContents, CitedActType, Citation, OjRef, Subparagraph};

#[test]
fn trademark_act_structure() {
    let act = load_act(Path::new("data/32017R1001"))
        .expect("failed to load TrademarkAct from data/32017R1001");
    let Act::Regular(reg) = act else { panic!("TrademarkAct should be a Regular act") };

    // Title must identify the act number.
    assert!(
        reg.title.contains("2017/1001"),
        "title did not contain '2017/1001': {}",
        reg.title
    );

    // Preamble: 4 legal bases, 48 recitals.
    assert_eq!(reg.preamble.visas.len(), 4, "unexpected visa count");
    assert_eq!(reg.preamble.recitals.len(), 48, "unexpected recital count");
    assert_eq!(
        reg.preamble.recitals[0].number, "(1)",
        "first recital number mismatch"
    );

    // Enacting terms: 14 chapters, 212 articles total.
    assert_eq!(reg.enacting_terms.chapters.len(), 14, "unexpected chapter count");

    let total_articles: usize = reg
        .enacting_terms
        .chapters
        .iter()
        .map(|c| match &c.contents {
            ChapterContents::Articles(arts) => arts.len(),
            ChapterContents::Sections(secs) => secs.iter().map(|s| s.articles.len()).sum(),
        })
        .sum();
    assert_eq!(total_articles, 212, "unexpected total article count");

    // 3 annexes, all identified as ANNEX I / II / III.
    assert_eq!(reg.annexes.len(), 3, "unexpected annex count");
    assert!(reg.annexes[0].number.contains("ANNEX I"), "annex 0: {}", reg.annexes[0].number);
    assert!(reg.annexes[1].number.contains("ANNEX II"), "annex 1: {}", reg.annexes[1].number);
    assert!(reg.annexes[2].number.contains("ANNEX III"), "annex 2: {}", reg.annexes[2].number);

    // Chapter I (idx 0): 3 direct articles.
    let ch1_arts = match &reg.enacting_terms.chapters[0].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter I should have direct articles"),
    };
    assert_eq!(ch1_arts.len(), 3, "Chapter I should have 3 articles");

    // Article 1 ("EU trade mark"): 2 paragraphs, first is a plain Paragraph.
    let art1 = &ch1_arts[0];
    assert_eq!(art1.number, "Article 1");
    assert_eq!(art1.title.as_deref(), Some("EU trade mark"));
    assert_eq!(art1.paragraphs.len(), 2);
    assert_eq!(art1.paragraphs[0].number.as_deref(), Some("1."));
    assert_eq!(art1.paragraphs[0].alineas.len(), 1);
    assert!(matches!(&art1.paragraphs[0].alineas[0], Subparagraph::Text { number: None, .. }));

    // Article 3 ("Capacity to act"): bare <ALINEA> (plain text, no block children)
    // → 1 unnamed paragraph with 1 plain Text block.
    let art3 = &ch1_arts[2];
    assert_eq!(art3.number, "Article 3");
    assert_eq!(art3.paragraphs.len(), 1);
    assert!(art3.paragraphs[0].number.is_none(), "bare-alinea paragraph should have no number");
    assert_eq!(art3.paragraphs[0].alineas.len(), 1);
    assert!(matches!(&art3.paragraphs[0].alineas[0], Subparagraph::Text { number: None, .. }));

    // Chapter II (idx 1): 4 sections.
    let ch2_secs = match &reg.enacting_terms.chapters[1].contents {
        ChapterContents::Sections(secs) => secs,
        _ => panic!("Chapter II should have sections"),
    };
    assert_eq!(ch2_secs.len(), 4, "Chapter II should have 4 sections");

    // Chapter II section 1 (idx 0): 5 articles; first is Article 4.
    // Article 7 ("Absolute grounds for refusal") is at index 3 of section 1.
    // Para 1 has <P> intro + 13 list items = 14 alinea blocks.
    let sec1_arts = &ch2_secs[0].articles;
    assert_eq!(sec1_arts.len(), 5, "Chapter II section 1 should have 5 articles");
    let art7 = &sec1_arts[3];
    assert_eq!(art7.number, "Article 7");
    assert_eq!(art7.title.as_deref(), Some("Absolute grounds for refusal"));
    let p1 = &art7.paragraphs[0];
    assert_eq!(p1.number.as_deref(), Some("1."));
    assert_eq!(p1.alineas.len(), 1, "Article 7 para 1 should be a single List block");
    match &p1.alineas[0] {
        Subparagraph::List(lb) => {
            assert_eq!(lb.items.len(), 13, "Article 7 para 1 list should have 13 items");
            assert!(matches!(&lb.items[0], Subparagraph::Text { number: Some(n), .. } if *n == 1));
        }
        _ => panic!("Article 7 para 1 alineas[0] should be a List"),
    }

    // Chapter V (idx 4): 5 direct articles.
    let ch5_arts = match &reg.enacting_terms.chapters[4].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter V should have direct articles"),
    };
    assert_eq!(ch5_arts.len(), 5, "Chapter V should have 5 articles");

    // Chapter XIV (idx 13): 6 direct articles (final chapter).
    let ch14_arts = match &reg.enacting_terms.chapters[13].contents {
        ChapterContents::Articles(arts) => arts,
        _ => panic!("Chapter XIV should have direct articles"),
    };
    assert_eq!(ch14_arts.len(), 6, "Chapter XIV should have 6 articles");

    // Annexes each wrap their content in GR.SEQ elements → AnnexContent::Sections.
    assert!(
        matches!(&reg.annexes[0].content, AnnexContent::Sections(_)),
        "Annex I content should be Sections (GR.SEQ)"
    );

    // No Definitions article → definitions map is empty.
    assert!(reg.definitions.is_empty(), "TrademarkAct should have no definitions");
}

#[test]
fn trademark_act_recital_citations() {
    let act = load_act(Path::new("data/32017R1001"))
        .expect("failed to load TrademarkAct from data/32017R1001");
    let Act::Regular(reg) = act else { panic!("TrademarkAct should be a Regular act") };

    let recitals = &reg.preamble.recitals;

    // Recital (1): cites Council Regulation (EC) No 207/2009 via NOTE with OJ ref.
    // Source: L_2017154EN.01000101.xml, first CONSID.
    assert_eq!(recitals[0].citations.len(), 1, "recital (1) should have exactly 1 citation");
    assert_eq!(
        recitals[0].citations[0],
        Citation {
            act_type: CitedActType::Regulation,
            regime: Some("EC".into()),
            number: "207/2009".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "078".into(), date: "20090324".into(), page: 1 }),
        },
        "recital (1): unexpected citation"
    );

    // Recital (2): three NOTE-backed citations plus one inline mention of 207/2009.
    // Source: L_2017154EN.01000101.xml, second CONSID (3 NOTEs for 40/94, 89/104, 2008/95;
    // "207/2009" also appears inline in the text body).
    let r2 = &recitals[1].citations;
    assert!(
        r2.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EC".into()), number: "40/94".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "011".into(), date: "19940114".into(), page: 1 }) }),
        "recital (2): missing (EC) No 40/94 with OJ ref"
    );
    assert!(
        r2.contains(&Citation { act_type: CitedActType::Directive, regime: Some("EEC".into()), number: "89/104".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "040".into(), date: "19890211".into(), page: 1 }) }),
        "recital (2): missing 89/104/EEC with OJ ref"
    );
    assert!(
        r2.contains(&Citation { act_type: CitedActType::Directive, regime: Some("EC".into()), number: "2008/95".into(),
            oj_ref: Some(OjRef { collection: "L".into(), number: "299".into(), date: "20081108".into(), page: 25 }) }),
        "recital (2): missing 2008/95/EC with OJ ref"
    );
    // Inline mention of 207/2009 in the recital body (no NOTE for it in recital (2)).
    assert!(
        r2.contains(&Citation { act_type: CitedActType::Regulation, regime: Some("EC".into()),
            number: "207/2009".into(), oj_ref: None }),
        "recital (2): missing inline 207/2009 (no OJ ref)"
    );

    // Recital (16): "(EU) No 608/2013" appears both inline and in a NOTE with OJ ref.
    // After deduplication, exactly one entry with the OJ ref must remain.
    // Source: L_2017154EN.01000101.xml, CONSID (16).
    let r16 = &recitals[15].citations;
    let eu_608: Vec<_> = r16.iter().filter(|c| c.number == "608/2013").collect();
    assert_eq!(eu_608.len(), 1, "recital (16): 608/2013 must appear exactly once");
    assert!(eu_608[0].oj_ref.is_some(), "recital (16): NOTE entry (with OJ ref) must win over inline");
    assert_eq!(eu_608[0].regime, Some("EU".into()));

    // Recital (18): "(EU) No 608/2013" inline only — no NOTE, so no OJ ref.
    // Source: L_2017154EN.01000101.xml, CONSID (18).
    let r18 = &recitals[17].citations;
    let eu_608_inline: Vec<_> = r18.iter().filter(|c| c.number == "608/2013").collect();
    assert_eq!(eu_608_inline.len(), 1, "recital (18): should have exactly one 608/2013 citation");
    assert!(eu_608_inline[0].oj_ref.is_none(), "recital (18): inline-only citation must have no OJ ref");
}
