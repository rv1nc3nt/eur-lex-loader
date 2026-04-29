use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;
use super::{child, parse_list};

/// Parses a Formex main-act XML string (`<ACT>` root) into its three parts.
///
/// Returns `(title, preamble, enacting_terms)`.  The caller is responsible for
/// combining these with parsed annexes to build a [`crate::model::Regulation`].
///
/// # Errors
///
/// Returns [`crate::error::Error::Xml`] for malformed XML and
/// [`crate::error::Error::MissingElement`] if `<TITLE>`, `<PREAMBLE>`, or
/// `<ENACTING.TERMS>` are absent from the document root.
pub fn parse_act(xml: &str) -> Result<(String, Preamble, EnactingTerms), Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();

    let title = parse_title(child(root, "TITLE")?)?;
    let preamble = parse_preamble(child(root, "PREAMBLE")?)?;
    let enacting_terms = parse_enacting_terms(child(root, "ENACTING.TERMS")?)?;

    Ok((title, preamble, enacting_terms))
}

/// Joins all `<P>` children of `<TITLE><TI>` into a single space-separated string.
fn parse_title(node: Node) -> Result<String, Error> {
    let ti = child(node, "TI")?;
    let parts: Vec<String> = ti
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "P")
        .map(extract_text)
        .collect();
    Ok(parts.join(" "))
}

/// Extracts all four structural parts of a `<PREAMBLE>` element.
fn parse_preamble(node: Node) -> Result<Preamble, Error> {
    let init = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "PREAMBLE.INIT")
        .map(extract_text)
        .unwrap_or_default();

    let visas = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "GR.VISA")
        .map(|gr| {
            gr.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "VISA")
                .map(extract_text)
                .collect()
        })
        .unwrap_or_default();

    let recitals = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "GR.CONSID")
        .map(|gr| {
            gr.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "CONSID")
                .map(parse_recital)
                .collect()
        })
        .unwrap_or_default();

    let enacting_formula = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "PREAMBLE.FINAL")
        .map(extract_text)
        .unwrap_or_default();

    Ok(Preamble { init, visas, recitals, enacting_formula })
}

/// Extracts the number and text from a single `<CONSID>` recital element.
///
/// Standard recitals use an `<NP>` wrapper with `<NO.P>` and `<TXT>` children.
/// If no `<NP>` is found the entire element is rendered as plain text with an
/// empty number.
fn parse_recital(node: Node) -> Recital {
    let np = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "NP");

    let (number, text) = if let Some(np) = np {
        let number = np
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "NO.P")
            .map(extract_text)
            .unwrap_or_default();
        let text = np
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "TXT")
            .map(extract_text)
            .unwrap_or_default();
        (number, text)
    } else {
        (String::new(), extract_text(node))
    };

    Recital { number, text }
}

/// Collects all top-level `<DIVISION>` elements as chapters.
fn parse_enacting_terms(node: Node) -> Result<EnactingTerms, Error> {
    let chapters = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "DIVISION")
        .map(parse_chapter)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EnactingTerms { chapters })
}

/// Parses a top-level `<DIVISION>` as a chapter.
///
/// If the division contains child `<DIVISION>` elements those are parsed as
/// sections; otherwise its `<ARTICLE>` children are parsed directly.
fn parse_chapter(node: Node) -> Result<Chapter, Error> {
    let title_node = child(node, "TITLE")?;
    let title = extract_text(child(title_node, "TI")?);
    let subtitle = title_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "STI")
        .map(extract_text);

    let sub_divisions: Vec<_> = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "DIVISION")
        .collect();

    let contents = if !sub_divisions.is_empty() {
        let sections = sub_divisions
            .into_iter()
            .map(parse_section)
            .collect::<Result<Vec<_>, _>>()?;
        ChapterContents::Sections(sections)
    } else {
        let articles = node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "ARTICLE")
            .map(parse_article)
            .collect::<Result<Vec<_>, _>>()?;
        ChapterContents::Articles(articles)
    };

    Ok(Chapter { title, subtitle, contents })
}

/// Parses a nested `<DIVISION>` as a section (articles only, no further nesting).
fn parse_section(node: Node) -> Result<Section, Error> {
    let title_node = child(node, "TITLE")?;
    let title = extract_text(child(title_node, "TI")?);
    let subtitle = title_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "STI")
        .map(extract_text);

    let articles = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ARTICLE")
        .map(parse_article)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Section { title, subtitle, articles })
}

/// Parses an `<ARTICLE>` element.
///
/// When `<PARAG>` wrappers are present each is parsed individually.  Some
/// articles (e.g. Article 113 of the EU AI Act) contain bare `<ALINEA>`
/// elements with no `<PARAG>` wrapper; those are collected into a single
/// [`Paragraph`] with `number: None`.
fn parse_article(node: Node) -> Result<Article, Error> {
    let number = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TI.ART")
        .map(extract_text)
        .unwrap_or_default();

    let title = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "STI.ART")
        .map(extract_text);

    let parag_nodes: Vec<_> = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "PARAG")
        .collect();

    let paragraphs = if !parag_nodes.is_empty() {
        parag_nodes
            .into_iter()
            .map(parse_paragraph)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        // Some articles have bare <ALINEA> children with no <PARAG> wrapper and
        // therefore no paragraph numbers. All alineas are grouped into a single
        // anonymous paragraph to keep the model uniform (every article has at
        // least one Paragraph). Real example: Article 3 of the DSA.
        let alineas: Vec<ContentBlock> = node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "ALINEA")
            .flat_map(expand_alinea)
            .collect();
        vec![Paragraph { number: None, alineas }]
    };

    Ok(Article { number, title, paragraphs })
}

/// Parses a `<PARAG>` element into a [`Paragraph`] with a number and alineas.
fn parse_paragraph(node: Node) -> Result<Paragraph, Error> {
    let number = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "NO.PARAG")
        .map(extract_text);

    let alineas: Vec<ContentBlock> = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ALINEA")
        .flat_map(expand_alinea)
        .collect();

    Ok(Paragraph { number, alineas })
}

/// Expands a single `<ALINEA>` element into one or more [`ContentBlock`]s.
///
/// - `<P>` children become [`ContentBlock::Paragraph`].
/// - `<LIST>` children are expanded via [`parse_list`] into
///   [`ContentBlock::ListItem`] entries (with nested sub-items when present).
/// - Falls back to a single `Paragraph` when the alinea has no recognised
///   block children (pure inline text, `<HT>`, `<QUOT.START>`, etc.).
fn expand_alinea(node: Node) -> Vec<ContentBlock> {
    let mut result: Vec<ContentBlock> = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "P" => {
                let t = extract_text(child);
                if !t.is_empty() {
                    result.push(ContentBlock::Paragraph(t));
                }
            }
            "LIST" => {
                result.extend(parse_list(child));
            }
            _ => {
                // Unrecognised block elements (e.g. <TABLE>, <FORMULA>) are
                // reduced to their text content. Structure is lost but no text
                // is silently dropped.
                let t = extract_text(child);
                if !t.is_empty() {
                    result.push(ContentBlock::Paragraph(t));
                }
            }
        }
    }
    // Pure inline alinea — wrap the whole text as a single Paragraph.
    if result.is_empty() {
        let t = extract_text(node);
        if !t.is_empty() {
            result.push(ContentBlock::Paragraph(t));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(xml: &str) -> roxmltree::Document<'_> {
        roxmltree::Document::parse(xml).unwrap()
    }

    // ── parse_act errors ──────────────────────────────────────────────────────

    #[test]
    fn parse_act_missing_title() {
        let result = parse_act("<ACT><PREAMBLE/><ENACTING.TERMS/></ACT>");
        assert!(matches!(result, Err(Error::MissingElement(_))));
    }

    #[test]
    fn parse_act_missing_preamble() {
        let result = parse_act(
            "<ACT><TITLE><TI><P>Title</P></TI></TITLE><ENACTING.TERMS/></ACT>",
        );
        assert!(matches!(result, Err(Error::MissingElement(_))));
    }

    #[test]
    fn parse_act_missing_enacting_terms() {
        let result = parse_act(
            "<ACT><TITLE><TI><P>Title</P></TI></TITLE><PREAMBLE/></ACT>",
        );
        assert!(matches!(result, Err(Error::MissingElement(_))));
    }

    // ── title ─────────────────────────────────────────────────────────────────

    #[test]
    fn title_joins_p_elements() {
        let xml = "<TITLE><TI><P>Regulation</P><P>of 1 January</P></TI></TITLE>";
        let d = doc(xml);
        let result = parse_title(d.root_element()).unwrap();
        assert_eq!(result, "Regulation of 1 January");
    }

    // ── preamble ──────────────────────────────────────────────────────────────

    #[test]
    fn preamble_counts_visas_and_recitals() {
        let xml = r#"<PREAMBLE>
            <PREAMBLE.INIT><P>THE COUNCIL,</P></PREAMBLE.INIT>
            <GR.VISA>
                <VISA><P>Visa one</P></VISA>
                <VISA><P>Visa two</P></VISA>
            </GR.VISA>
            <GR.CONSID>
                <CONSID><NP><NO.P>(1)</NO.P><TXT>First recital.</TXT></NP></CONSID>
                <CONSID><NP><NO.P>(2)</NO.P><TXT>Second recital.</TXT></NP></CONSID>
                <CONSID><NP><NO.P>(3)</NO.P><TXT>Third recital.</TXT></NP></CONSID>
            </GR.CONSID>
            <PREAMBLE.FINAL><P>HAVE ADOPTED:</P></PREAMBLE.FINAL>
        </PREAMBLE>"#;
        let d = doc(xml);
        let p = parse_preamble(d.root_element()).unwrap();
        assert_eq!(p.visas.len(), 2);
        assert_eq!(p.recitals.len(), 3);
        assert_eq!(p.init, "THE COUNCIL,");
        assert_eq!(p.enacting_formula, "HAVE ADOPTED:");
    }

    #[test]
    fn recital_number_and_text() {
        let xml = "<CONSID><NP><NO.P>(42)</NO.P><TXT>Some text.</TXT></NP></CONSID>";
        let d = doc(xml);
        let r = parse_recital(d.root_element());
        assert_eq!(r.number, "(42)");
        assert_eq!(r.text, "Some text.");
    }

    #[test]
    fn recital_without_np_falls_back_to_full_text() {
        let xml = "<CONSID><P>Unnumbered recital.</P></CONSID>";
        let d = doc(xml);
        let r = parse_recital(d.root_element());
        assert_eq!(r.number, "");
        assert_eq!(r.text, "Unnumbered recital.");
    }

    // ── chapters and sections ─────────────────────────────────────────────────

    #[test]
    fn chapter_with_direct_articles() {
        let xml = r#"<DIVISION>
            <TITLE><TI><P>CHAPTER I</P></TI></TITLE>
            <ARTICLE><TI.ART>Article 1</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
            <ARTICLE><TI.ART>Article 2</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
        </DIVISION>"#;
        let d = doc(xml);
        let ch = parse_chapter(d.root_element()).unwrap();
        assert_eq!(ch.title, "CHAPTER I");
        match ch.contents {
            ChapterContents::Articles(arts) => assert_eq!(arts.len(), 2),
            ChapterContents::Sections(_) => panic!("expected Articles"),
        }
    }

    #[test]
    fn chapter_with_sections() {
        let xml = r#"<DIVISION>
            <TITLE><TI><P>CHAPTER III</P></TI></TITLE>
            <DIVISION>
                <TITLE><TI><P>SECTION 1</P></TI></TITLE>
                <ARTICLE><TI.ART>Article 5</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
            </DIVISION>
            <DIVISION>
                <TITLE><TI><P>SECTION 2</P></TI></TITLE>
                <ARTICLE><TI.ART>Article 6</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
            </DIVISION>
        </DIVISION>"#;
        let d = doc(xml);
        let ch = parse_chapter(d.root_element()).unwrap();
        match ch.contents {
            ChapterContents::Sections(secs) => {
                assert_eq!(secs.len(), 2);
                assert_eq!(secs[0].title, "SECTION 1");
                assert_eq!(secs[1].articles.len(), 1);
            }
            ChapterContents::Articles(_) => panic!("expected Sections"),
        }
    }

    // ── articles ──────────────────────────────────────────────────────────────

    #[test]
    fn article_with_paragraphs() {
        let xml = r#"<ARTICLE>
            <TI.ART>Article 6</TI.ART>
            <STI.ART><P>Classification rules</P></STI.ART>
            <PARAG><NO.PARAG>1.</NO.PARAG><ALINEA>First paragraph.</ALINEA></PARAG>
            <PARAG><NO.PARAG>2.</NO.PARAG><ALINEA>Second paragraph.</ALINEA></PARAG>
        </ARTICLE>"#;
        let d = doc(xml);
        let art = parse_article(d.root_element()).unwrap();
        assert_eq!(art.number, "Article 6");
        assert_eq!(art.title.as_deref(), Some("Classification rules"));
        assert_eq!(art.paragraphs.len(), 2);
        assert_eq!(art.paragraphs[0].number.as_deref(), Some("1."));
        assert!(matches!(&art.paragraphs[1].alineas[0],
            ContentBlock::Paragraph(t) if t == "Second paragraph."));
    }

    #[test]
    fn article_bare_alineas_become_single_paragraph() {
        // Some articles have no <PARAG> wrapper — alineas sit directly under <ARTICLE>.
        let xml = r#"<ARTICLE>
            <TI.ART>Article 113</TI.ART>
            <ALINEA>Only text.</ALINEA>
        </ARTICLE>"#;
        let d = doc(xml);
        let art = parse_article(d.root_element()).unwrap();
        assert_eq!(art.paragraphs.len(), 1);
        assert!(art.paragraphs[0].number.is_none());
        assert!(matches!(&art.paragraphs[0].alineas[0], ContentBlock::Paragraph(t) if t == "Only text."));
    }

    #[test]
    fn alinea_list_items_are_content_blocks() {
        // A <LIST> inside an <ALINEA> must produce ContentBlock::ListItem entries,
        // not plain strings — matching Article 5's prohibited-practices list.
        let xml = r#"<ARTICLE>
            <TI.ART>Article 5</TI.ART>
            <PARAG>
                <NO.PARAG>1.</NO.PARAG>
                <ALINEA>
                    <P>The following shall be prohibited:</P>
                    <LIST TYPE="alpha">
                        <ITEM><NP><NO.P>(a)</NO.P><TXT>Practice A.</TXT></NP></ITEM>
                        <ITEM><NP>
                            <NO.P>(b)</NO.P>
                            <TXT>Practice B:</TXT>
                            <P><LIST TYPE="roman">
                                <ITEM><NP><NO.P>(i)</NO.P><TXT>Sub-practice i.</TXT></NP></ITEM>
                                <ITEM><NP><NO.P>(ii)</NO.P><TXT>Sub-practice ii.</TXT></NP></ITEM>
                            </LIST></P>
                        </NP></ITEM>
                    </LIST>
                </ALINEA>
                <ALINEA>Point (b) is without prejudice to existing rules.</ALINEA>
            </PARAG>
        </ARTICLE>"#;
        let d = doc(xml);
        let art = parse_article(d.root_element()).unwrap();
        assert_eq!(art.paragraphs.len(), 1);
        let alineas = &art.paragraphs[0].alineas;
        // Paragraph intro + 2 list items + plain trailing alinea = 4 blocks.
        assert_eq!(alineas.len(), 4);
        assert!(matches!(&alineas[0], ContentBlock::Paragraph(t) if t.contains("prohibited")));
        assert!(matches!(&alineas[1], ContentBlock::ListItem { number, sub_items, .. }
            if number == "(a)" && sub_items.is_empty()));
        match &alineas[2] {
            ContentBlock::ListItem { number, text, sub_items } => {
                assert_eq!(number, "(b)");
                assert_eq!(text, "Practice B:");
                assert_eq!(sub_items.len(), 2);
                assert!(matches!(&sub_items[0], ContentBlock::ListItem { number, .. } if number == "(i)"));
                assert!(matches!(&sub_items[1], ContentBlock::ListItem { number, .. } if number == "(ii)"));
            }
            _ => panic!("expected ListItem at alineas[2]"),
        }
        assert!(matches!(&alineas[3], ContentBlock::Paragraph(t) if t.contains("prejudice")));
    }

    #[test]
    fn alinea_list_expands_to_individual_alineas() {
        // An <ALINEA> that contains a <P> intro and a <LIST> should yield one
        // alinea for the intro and one per <ITEM>, not a single flattened string.
        // This matches Article 3 of the EU AI Act (definitions).
        let xml = r#"<ARTICLE>
            <TI.ART>Article 3</TI.ART>
            <STI.ART><P>Definitions</P></STI.ART>
            <ALINEA>
                <P>For the purposes of this Regulation:</P>
                <LIST TYPE="ARAB">
                    <ITEM><NP><NO.P>(1)</NO.P><TXT>first definition</TXT></NP></ITEM>
                    <ITEM><NP><NO.P>(2)</NO.P><TXT>second definition</TXT></NP></ITEM>
                </LIST>
            </ALINEA>
        </ARTICLE>"#;
        let d = doc(xml);
        let art = parse_article(d.root_element()).unwrap();
        assert_eq!(art.paragraphs.len(), 1);
        let alineas = &art.paragraphs[0].alineas;
        // intro + 2 list items = 3 alineas
        assert_eq!(alineas.len(), 3);
        assert!(matches!(&alineas[0], ContentBlock::Paragraph(t) if t == "For the purposes of this Regulation:"));
        assert!(matches!(&alineas[1], ContentBlock::ListItem { number, text, .. }
            if number == "(1)" && text == "first definition"));
        assert!(matches!(&alineas[2], ContentBlock::ListItem { number, text, .. }
            if number == "(2)" && text == "second definition"));
    }
}
