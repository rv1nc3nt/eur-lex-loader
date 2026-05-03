//! Parser for Formex act XML files (`<ACT>` and `<CONS.ACT>` roots).
//!
//! The two public functions, [`parse_regular_act`] and [`parse_consolidated_act`],
//! each parse one `.fmx.xml` file and return `(title, preamble, enacting_terms)`.
//! The caller ([`crate::loader`]) assembles those parts with annex data into a
//! complete [`crate::model::Act`].

use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use super::text::extract_text;
use super::{child, extract_citations, parse_block_children};

/// Parses a regular Formex act XML string (`<ACT>` root) into its three parts.
///
/// Returns `(title, preamble, enacting_terms)`. The caller assembles these with
/// parsed annex files to build a [`crate::model::RegularAct`].
///
/// # Errors
///
/// Returns [`crate::error::Error::Xml`] for malformed XML and
/// [`crate::error::Error::MissingElement`] if `<TITLE>`, `<PREAMBLE>`, or
/// `<ENACTING.TERMS>` are absent from the document root.
pub fn parse_regular_act(xml: &str) -> Result<(String, Preamble, EnactingTerms), Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    let title = parse_title(child(root, "TITLE")?)?;
    let preamble = parse_preamble(child(root, "PREAMBLE")?)?;
    let enacting_terms = parse_enacting_terms(child(root, "ENACTING.TERMS")?)?;
    Ok((title, preamble, enacting_terms))
}

/// Parses a consolidated Formex act XML string (`<CONS.ACT>` root) into its three parts.
///
/// Returns `(title, preamble, enacting_terms)`. The caller assembles these with
/// inline `<CONS.ANNEX>` elements to build a [`crate::model::ConsolidatedAct`].
///
/// # Errors
///
/// Returns [`crate::error::Error::Xml`] for malformed XML and
/// [`crate::error::Error::MissingElement`] if `<CONS.DOC>`, `<TITLE>`,
/// `<PREAMBLE>`, or `<ENACTING.TERMS>` are absent.
pub fn parse_consolidated_act(xml: &str) -> Result<(String, ConsolidatedPreamble, EnactingTerms), Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    let content = child(root, "CONS.DOC")?;
    let title = parse_title(child(content, "TITLE")?)?;
    let preamble = parse_consolidated_preamble(child(content, "PREAMBLE")?)?;
    let enacting_terms = parse_enacting_terms(child(content, "ENACTING.TERMS")?)?;
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

/// Extracts the two fields of a consolidated preamble (no visas or recitals).
fn parse_consolidated_preamble(node: Node) -> Result<ConsolidatedPreamble, Error> {
    let init = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "PREAMBLE.INIT")
        .map(extract_text)
        .unwrap_or_default();
    let enacting_formula = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "PREAMBLE.FINAL")
        .map(extract_text)
        .unwrap_or_default();
    Ok(ConsolidatedPreamble { init, enacting_formula })
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

    let citations = extract_citations(node);
    Recital { number, text, citations }
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
        let alineas: Vec<Subparagraph> = node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "ALINEA")
            .flat_map(expand_alinea)
            .collect();
        vec![Paragraph { number: None, alineas, citations: extract_citations(node) }]
    };

    Ok(Article { number, title, paragraphs })
}

/// Parses a `<PARAG>` element into a [`Paragraph`] with a number and alineas.
fn parse_paragraph(node: Node) -> Result<Paragraph, Error> {
    let number = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "NO.PARAG")
        .map(extract_text);

    let alineas: Vec<Subparagraph> = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ALINEA")
        .flat_map(expand_alinea)
        .collect();

    let citations = extract_citations(node);
    Ok(Paragraph { number, alineas, citations })
}

/// Expands a single `<ALINEA>` element into one or more [`Subparagraph`]s.
fn expand_alinea(node: Node) -> Vec<Subparagraph> {
    parse_block_children(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses a raw XML string into a `roxmltree::Document`, panicking on error.
    fn doc(xml: &str) -> roxmltree::Document<'_> {
        roxmltree::Document::parse(xml).unwrap()
    }

    // ── parse_act errors ──────────────────────────────────────────────────────

    #[test]
    /// An `<ACT>` without a `<TITLE>` element returns a `MissingElement` error.
    fn parse_act_missing_title() {
        let result = parse_regular_act("<ACT><PREAMBLE/><ENACTING.TERMS/></ACT>");
        assert!(matches!(result, Err(Error::MissingElement(_))));
    }

    // ── consolidated act (CONS.ACT) ───────────────────────────────────────────

    #[test]
    /// A minimal `<CONS.ACT>` with one `<DIVISION>` is parsed into title, preamble, and enacting terms.
    fn parse_cons_act_basic() {
        let xml = r#"<CONS.ACT>
            <INFO.CONSLEG/>
            <INFO.PROD/>
            <CONS.DOC>
                <BIB.INSTANCE/>
                <FAM.COMP/>
                <TITLE><TI><P>Test Consolidated Regulation</P></TI></TITLE>
                <PREAMBLE>
                    <PREAMBLE.INIT><P>THE COUNCIL,</P></PREAMBLE.INIT>
                    <PREAMBLE.FINAL><P>HAVE ADOPTED:</P></PREAMBLE.FINAL>
                </PREAMBLE>
                <ENACTING.TERMS>
                    <DIVISION>
                        <TITLE><TI><P>TITLE I</P></TI></TITLE>
                        <ARTICLE><TI.ART>Article 1</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
                    </DIVISION>
                </ENACTING.TERMS>
            </CONS.DOC>
        </CONS.ACT>"#;
        let (title, preamble, enacting_terms) = parse_consolidated_act(xml).unwrap();
        assert_eq!(title, "Test Consolidated Regulation");
        assert_eq!(preamble.init, "THE COUNCIL,");
        assert_eq!(preamble.enacting_formula, "HAVE ADOPTED:");
        assert_eq!(enacting_terms.chapters.len(), 1);
    }

    #[test]
    /// A `<TOC>` element inside `<ENACTING.TERMS>` is ignored; only `<DIVISION>` elements become chapters.
    fn parse_cons_act_toc_is_skipped() {
        // The <TOC> element inside <ENACTING.TERMS> of a consolidated act must
        // not be counted as a chapter — only <DIVISION> elements are chapters.
        let xml = r#"<CONS.ACT>
            <INFO.CONSLEG/>
            <CONS.DOC>
                <TITLE><TI><P>Act</P></TI></TITLE>
                <PREAMBLE>
                    <PREAMBLE.INIT><P>Init.</P></PREAMBLE.INIT>
                    <PREAMBLE.FINAL><P>Final.</P></PREAMBLE.FINAL>
                </PREAMBLE>
                <ENACTING.TERMS>
                    <TOC><TITLE><TI><P>Table of Contents</P></TI></TITLE></TOC>
                    <DIVISION>
                        <TITLE><TI><P>TITLE I</P></TI></TITLE>
                        <ARTICLE><TI.ART>Article 1</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
                    </DIVISION>
                    <DIVISION>
                        <TITLE><TI><P>TITLE II</P></TI></TITLE>
                        <ARTICLE><TI.ART>Article 2</TI.ART><ALINEA>Text.</ALINEA></ARTICLE>
                    </DIVISION>
                </ENACTING.TERMS>
            </CONS.DOC>
        </CONS.ACT>"#;
        let (_, _, enacting_terms) = parse_consolidated_act(xml).unwrap();
        assert_eq!(enacting_terms.chapters.len(), 2, "TOC must not be counted as a chapter");
    }

    #[test]
    /// An `<ACT>` without a `<PREAMBLE>` element returns a `MissingElement` error.
    fn parse_act_missing_preamble() {
        let result = parse_regular_act(
            "<ACT><TITLE><TI><P>Title</P></TI></TITLE><ENACTING.TERMS/></ACT>",
        );
        assert!(matches!(result, Err(Error::MissingElement(_))));
    }

    #[test]
    /// An `<ACT>` without an `<ENACTING.TERMS>` element returns a `MissingElement` error.
    fn parse_act_missing_enacting_terms() {
        let result = parse_regular_act(
            "<ACT><TITLE><TI><P>Title</P></TI></TITLE><PREAMBLE/></ACT>",
        );
        assert!(matches!(result, Err(Error::MissingElement(_))));
    }

    // ── title ─────────────────────────────────────────────────────────────────

    #[test]
    /// Multiple `<P>` children inside `<TI>` are joined with a space into a single title string.
    fn title_joins_p_elements() {
        let xml = "<TITLE><TI><P>Act</P><P>of 1 January</P></TI></TITLE>";
        let d = doc(xml);
        let result = parse_title(d.root_element()).unwrap();
        assert_eq!(result, "Act of 1 January");
    }

    // ── preamble ──────────────────────────────────────────────────────────────

    #[test]
    /// Preamble with two `<VISA>` and three `<CONSID>` elements produces the correct counts and texts.
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
    /// A `<CONSID>` with `<NO.P>` and `<TXT>` produces the correct recital number and text.
    fn recital_number_and_text() {
        let xml = "<CONSID><NP><NO.P>(42)</NO.P><TXT>Some text.</TXT></NP></CONSID>";
        let d = doc(xml);
        let r = parse_recital(d.root_element());
        assert_eq!(r.number, "(42)");
        assert_eq!(r.text, "Some text.");
    }

    #[test]
    /// A `<CONSID>` with no `<NP>` wrapper falls back to rendering the whole element
    /// as plain text with an empty number string.
    fn recital_without_np_falls_back_to_full_text() {
        let xml = "<CONSID><P>Unnumbered recital.</P></CONSID>";
        let d = doc(xml);
        let r = parse_recital(d.root_element());
        assert_eq!(r.number, "");
        assert_eq!(r.text, "Unnumbered recital.");
    }

    // ── chapters and sections ─────────────────────────────────────────────────

    #[test]
    /// A `<DIVISION>` with only `<ARTICLE>` children (no nested `<DIVISION>`) produces
    /// `ChapterContents::Articles`.
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
    /// A `<DIVISION>` whose children are themselves `<DIVISION>` elements produces
    /// `ChapterContents::Sections`, each section carrying its own articles.
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
    /// An `<ARTICLE>` with `<PARAG>` wrappers, a `<TI.ART>` number, and a
    /// `<STI.ART>` subtitle is parsed into the correct counts and field values.
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
            Subparagraph::Text { text: t, number: None } if t == "Second paragraph."));
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
        assert!(matches!(&art.paragraphs[0].alineas[0], Subparagraph::Text { text: t, number: None } if t == "Only text."));
    }

    #[test]
    /// A `<LIST>` inside an `<ALINEA>` must produce a `Subparagraph::List` (not flat
    /// text), including correct nesting for sub-lists — matching Article 5's
    /// prohibited-practices list structure.
    fn alinea_list_items_are_content_blocks() {
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
        // Intro+list grouped into one List block + plain trailing alinea = 2 blocks.
        assert_eq!(alineas.len(), 2);
        match &alineas[0] {
            Subparagraph::List(lb) => {
                assert!(lb.intro.contains("prohibited"));
                assert_eq!(lb.items.len(), 2);
                assert!(matches!(&lb.items[0], Subparagraph::Text { number: Some(n), .. } if n == "(a)"));
                match &lb.items[1] {
                    Subparagraph::List(inner) => {
                        assert_eq!(inner.number.as_deref(), Some("(b)"));
                        assert_eq!(inner.intro, "Practice B:");
                        assert_eq!(inner.items.len(), 2);
                        assert!(matches!(&inner.items[0], Subparagraph::Text { number: Some(n), .. } if n == "(i)"));
                        assert!(matches!(&inner.items[1], Subparagraph::Text { number: Some(n), .. } if n == "(ii)"));
                    }
                    _ => panic!("expected nested List for item (b)"),
                }
            }
            _ => panic!("expected List at alineas[0]"),
        }
        assert!(matches!(&alineas[1], Subparagraph::Text { text: t, number: None } if t.contains("prejudice")));
    }

    #[test]
    /// An `<ALINEA>` containing a `<P>` intro followed by a `<LIST>` must produce a
    /// single `Subparagraph::List` with the intro set and items populated —
    /// matching Article 3 of the EU AI Act (definitions article).
    fn alinea_list_expands_to_individual_alineas() {
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
        // Intro+list collapsed into a single List block.
        assert_eq!(alineas.len(), 1);
        match &alineas[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.intro, "For the purposes of this Regulation:");
                assert_eq!(lb.items.len(), 2);
                assert!(matches!(&lb.items[0], Subparagraph::Text { text: t, number: Some(n) } if n == "(1)" && t == "first definition"));
                assert!(matches!(&lb.items[1], Subparagraph::Text { text: t, number: Some(n) } if n == "(2)" && t == "second definition"));
            }
            _ => panic!("expected List at alineas[0]"),
        }
    }
}
