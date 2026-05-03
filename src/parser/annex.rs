use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;
use super::{child, parse_block_children, parse_list_as_subparagraphs};

/// Parses a Formex annex XML string (`<ANNEX>` root) into an [`Annex`].
///
/// If the `<CONTENTS>` element's top-level children include any `<GR.SEQ>`
/// elements the annex is parsed as [`AnnexContent::Sections`]; otherwise it is
/// parsed as [`AnnexContent::Paragraphs`], treating each `<NP>` as a
/// [`Paragraph`] and grouping surrounding `<P>`/`<LIST>` elements into
/// anonymous paragraphs.
///
/// # Errors
///
/// Returns [`crate::error::Error::Xml`] for malformed XML and
/// [`crate::error::Error::MissingElement`] if `<TITLE>` is absent.
pub fn parse_annex(xml: &str) -> Result<Annex, Error> {
    let doc = Document::parse(xml)?;
    parse_annex_node(doc.root_element())
}

/// Parses all `<CONS.ANNEX>` elements embedded in a `<CONS.ACT>` document,
/// returning them in document order.
///
/// Consolidated acts keep their annexes inline inside `<CONS.DOC>` rather
/// than as separate files. Each `<CONS.ANNEX>` has the same `<TITLE>` +
/// `<CONTENTS>` structure as a standalone `<ANNEX>` and is parsed identically.
///
/// # Errors
///
/// Returns [`crate::error::Error::Xml`] for malformed XML and
/// [`crate::error::Error::MissingElement`] if `<CONS.DOC>` is absent.
pub fn parse_cons_annex(xml: &str) -> Result<Vec<Annex>, Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    let cons_doc = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "CONS.DOC")
        .ok_or(Error::MissingElement("CONS.DOC"))?;
    cons_doc
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "CONS.ANNEX")
        .map(parse_annex_node)
        .collect()
}

/// Parses a single annex node — works for both `<ANNEX>` and `<CONS.ANNEX>`.
fn parse_annex_node(root: Node) -> Result<Annex, Error> {
    let title_node = child(root, "TITLE")?;

    let number = title_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TI")
        .map(extract_text)
        .unwrap_or_default();

    let subtitle = title_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "STI")
        .map(extract_text);

    let content = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "CONTENTS")
        .map(|contents| {
            let has_sections = contents
                .children()
                .any(|n| n.is_element() && n.tag_name().name() == "GR.SEQ");
            if has_sections {
                AnnexContent::Sections(parse_annex_sections(contents))
            } else {
                AnnexContent::Paragraphs(parse_annex_paragraphs(contents))
            }
        })
        .unwrap_or(AnnexContent::Paragraphs(vec![]));

    Ok(Annex { number, subtitle, content })
}

/// Collects all top-level `<GR.SEQ>` children of `node` as [`AnnexSection`]s.
fn parse_annex_sections(node: Node) -> Vec<AnnexSection> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "GR.SEQ")
        .map(|gr| {
            let title = gr
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "TITLE")
                .and_then(|t| t.children().find(|n| n.is_element() && n.tag_name().name() == "TI"))
                .map(extract_text)
                .unwrap_or_default();
            AnnexSection { title, alineas: parse_block_children(gr) }
        })
        .collect()
}

/// Parses flat annex content as a sequence of [`Paragraph`]s.
///
/// Each top-level `<NP>` becomes a `Paragraph` with its `<NO.P>` as the number
/// and its `<TXT>` / nested `<P><LIST>` as alineas.  Runs of `<P>` and `<LIST>`
/// between `<NP>` elements are collected into anonymous paragraphs.
fn parse_annex_paragraphs(node: Node) -> Vec<Paragraph> {
    let mut result: Vec<Paragraph> = Vec::new();
    let mut pending_alineas: Vec<Subparagraph> = Vec::new();
    let mut pending_intro: Option<String> = None;

    let flush_pending = |result: &mut Vec<Paragraph>,
                         alineas: &mut Vec<Subparagraph>,
                         intro: &mut Option<String>| {
        if let Some(t) = intro.take() {
            alineas.push(Subparagraph::Text { text: t, number: None });
        }
        if !alineas.is_empty() {
            result.push(Paragraph { number: None, alineas: std::mem::take(alineas) });
        }
    };

    for elem in node.children().filter(|n| n.is_element()) {
        match elem.tag_name().name() {
            "NP" => {
                flush_pending(&mut result, &mut pending_alineas, &mut pending_intro);
                result.push(np_to_paragraph(elem));
            }
            "P" => {
                let nested_lists: Vec<_> = elem
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "LIST")
                    .collect();
                if !nested_lists.is_empty() {
                    if let Some(t) = pending_intro.take() {
                        pending_alineas.push(Subparagraph::Text { text: t, number: None });
                    }
                    for list in nested_lists {
                        pending_alineas.push(Subparagraph::List(ListBlock {
                            number: None,
                            intro: String::new(),
                            items: parse_list_as_subparagraphs(list),
                        }));
                    }
                } else {
                    if let Some(t) = pending_intro.take() {
                        pending_alineas.push(Subparagraph::Text { text: t, number: None });
                    }
                    let t = extract_text(elem);
                    if !t.is_empty() {
                        pending_intro = Some(t);
                    }
                }
            }
            "LIST" => {
                let intro = pending_intro.take().unwrap_or_default();
                pending_alineas.push(Subparagraph::List(ListBlock {
                    number: None,
                    intro,
                    items: parse_list_as_subparagraphs(elem),
                }));
            }
            "TITLE" => {}
            _ => {
                if let Some(t) = pending_intro.take() {
                    pending_alineas.push(Subparagraph::Text { text: t, number: None });
                }
                let t = extract_text(elem);
                if !t.is_empty() {
                    pending_intro = Some(t);
                }
            }
        }
    }

    flush_pending(&mut result, &mut pending_alineas, &mut pending_intro);
    result
}

/// Converts a single `<NP>` element into a [`Paragraph`].
///
/// `<NO.P>` becomes the paragraph number, `<TXT>` becomes the first alinea,
/// and any `<P><LIST>` nested inside the `<NP>` become additional alineas.
fn np_to_paragraph(node: Node) -> Paragraph {
    let number = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "NO.P")
        .map(extract_text);

    let txt = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TXT")
        .map(extract_text)
        .unwrap_or_default();

    let nested: Vec<Subparagraph> = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "P")
        .flat_map(|p| {
            p.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "LIST")
                .flat_map(parse_list_as_subparagraphs)
        })
        .collect();

    let alineas = if nested.is_empty() {
        if txt.is_empty() {
            vec![]
        } else {
            vec![Subparagraph::Text { text: txt, number: None }]
        }
    } else {
        vec![Subparagraph::List(ListBlock { number: None, intro: txt, items: nested })]
    };

    Paragraph { number, alineas }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_sections(contents_inner: &str) -> Vec<AnnexSection> {
        let xml = format!(
            r#"<ANNEX><TITLE><TI><P>ANNEX X</P></TI></TITLE><CONTENTS>{}</CONTENTS></ANNEX>"#,
            contents_inner
        );
        match parse_annex(&xml).unwrap().content {
            AnnexContent::Sections(s) => s,
            AnnexContent::Paragraphs(_) => panic!("expected Sections"),
        }
    }

    fn parse_paragraphs(contents_inner: &str) -> Vec<Paragraph> {
        let xml = format!(
            r#"<ANNEX><TITLE><TI><P>ANNEX X</P></TI></TITLE><CONTENTS>{}</CONTENTS></ANNEX>"#,
            contents_inner
        );
        match parse_annex(&xml).unwrap().content {
            AnnexContent::Paragraphs(p) => p,
            AnnexContent::Sections(_) => panic!("expected Paragraphs"),
        }
    }

    // ── parse_annex errors ────────────────────────────────────────────────────

    #[test]
    fn parse_annex_missing_title() {
        let result = parse_annex("<ANNEX><CONTENTS/></ANNEX>");
        assert!(matches!(result, Err(crate::error::Error::MissingElement(_))));
    }

    // ── parse_cons_annex ────────────────────────────────────────────────

    #[test]
    fn parse_cons_annex_basic() {
        let xml = r#"<CONS.ACT>
            <INFO.CONSLEG/>
            <CONS.DOC>
                <TITLE><TI><P>Act</P></TI></TITLE>
                <CONS.ANNEX>
                    <TITLE><TI><P>ANNEX I</P></TI><STI><P>Subtitle</P></STI></TITLE>
                    <CONTENTS><P>Some content.</P></CONTENTS>
                </CONS.ANNEX>
                <CONS.ANNEX>
                    <TITLE><TI><P>ANNEX II</P></TI></TITLE>
                    <CONTENTS>
                        <GR.SEQ>
                            <TITLE><TI><P>Part A</P></TI></TITLE>
                            <P>Content.</P>
                        </GR.SEQ>
                    </CONTENTS>
                </CONS.ANNEX>
            </CONS.DOC>
        </CONS.ACT>"#;
        let annexes = parse_cons_annex(xml).unwrap();
        assert_eq!(annexes.len(), 2);
        assert_eq!(annexes[0].number, "ANNEX I");
        assert_eq!(annexes[0].subtitle.as_deref(), Some("Subtitle"));
        assert!(matches!(&annexes[0].content, AnnexContent::Paragraphs(_)));
        assert_eq!(annexes[1].number, "ANNEX II");
        assert!(matches!(&annexes[1].content, AnnexContent::Sections(s) if s.len() == 1));
    }

    #[test]
    fn parse_cons_annex_empty_when_no_cons_annex() {
        let xml = r#"<CONS.ACT>
            <CONS.DOC>
                <TITLE><TI><P>Act</P></TI></TITLE>
            </CONS.DOC>
        </CONS.ACT>"#;
        let annexes = parse_cons_annex(xml).unwrap();
        assert!(annexes.is_empty());
    }

    // ── parse_annex ───────────────────────────────────────────────────────────

    #[test]
    fn annex_title_and_subtitle() {
        let xml = r#"<ANNEX>
            <TITLE>
                <TI><P>ANNEX I</P></TI>
                <STI><P>List of legislation</P></STI>
            </TITLE>
            <CONTENTS/>
        </ANNEX>"#;
        let annex = parse_annex(xml).unwrap();
        assert_eq!(annex.number, "ANNEX I");
        assert_eq!(annex.subtitle.as_deref(), Some("List of legislation"));
    }

    #[test]
    fn annex_no_subtitle() {
        let xml = r#"<ANNEX>
            <TITLE><TI><P>ANNEX II</P></TI></TITLE>
            <CONTENTS/>
        </ANNEX>"#;
        let annex = parse_annex(xml).unwrap();
        assert!(annex.subtitle.is_none());
    }

    // ── Sections mode ─────────────────────────────────────────────────────────

    #[test]
    fn gr_seq_produces_annex_sections() {
        let xml = r#"<GR.SEQ>
            <TITLE><TI><P>Part A</P></TI></TITLE>
            <P>Content paragraph.</P>
        </GR.SEQ>
        <GR.SEQ>
            <TITLE><TI><P>Part B</P></TI></TITLE>
            <P>Another paragraph.</P>
        </GR.SEQ>"#;
        let sections = parse_sections(xml);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].title, "Part A");
        assert_eq!(sections[0].alineas.len(), 1);
        assert!(matches!(&sections[0].alineas[0], Subparagraph::Text { text: t, .. } if t == "Content paragraph."));
        assert_eq!(sections[1].title, "Part B");
    }

    #[test]
    fn gr_seq_with_list_inside() {
        let xml = r#"<GR.SEQ>
            <TITLE><TI><P>Section I</P></TI></TITLE>
            <P>Intro text.</P>
            <LIST TYPE="alpha">
                <ITEM><NP><NO.P>(a)</NO.P><TXT>Item a.</TXT></NP></ITEM>
                <ITEM><NP><NO.P>(b)</NO.P><TXT>Item b.</TXT></NP></ITEM>
            </LIST>
        </GR.SEQ>"#;
        let sections = parse_sections(xml);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].alineas.len(), 1);
        match &sections[0].alineas[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.intro, "Intro text.");
                assert_eq!(lb.items.len(), 2);
            }
            _ => panic!("expected List"),
        }
    }

    // ── Paragraphs mode ───────────────────────────────────────────────────────

    #[test]
    fn plain_paragraphs_become_anonymous_paragraph() {
        let paras = parse_paragraphs("<P>Some text.</P>");
        assert_eq!(paras.len(), 1);
        assert!(paras[0].number.is_none());
        assert_eq!(paras[0].alineas.len(), 1);
        assert!(matches!(&paras[0].alineas[0],
            Subparagraph::Text { text: t, number: None } if t == "Some text."));
    }

    #[test]
    fn empty_contents_produces_no_paragraphs() {
        let paras = parse_paragraphs("<P>   </P>");
        assert_eq!(paras.len(), 0);
    }

    #[test]
    fn np_becomes_numbered_paragraph() {
        let paras = parse_paragraphs("<NP><NO.P>1.</NO.P><TXT>First item.</TXT></NP>");
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0].number.as_deref(), Some("1."));
        assert_eq!(paras[0].alineas.len(), 1);
        assert!(matches!(&paras[0].alineas[0],
            Subparagraph::Text { text: t, number: None } if t == "First item."));
    }

    #[test]
    fn multiple_nps_become_separate_paragraphs() {
        let xml = r#"<NP><NO.P>1.</NO.P><TXT>First.</TXT></NP>
                     <NP><NO.P>2.</NO.P><TXT>Second.</TXT></NP>"#;
        let paras = parse_paragraphs(xml);
        assert_eq!(paras.len(), 2);
        assert_eq!(paras[0].number.as_deref(), Some("1."));
        assert_eq!(paras[1].number.as_deref(), Some("2."));
    }

    #[test]
    fn np_with_nested_list_becomes_list_alinea() {
        let xml = r#"<NP>
            <NO.P>1.</NO.P>
            <TXT>The following apply:</TXT>
            <P><LIST TYPE="alpha">
                <ITEM><NP><NO.P>(a)</NO.P><TXT>Item a.</TXT></NP></ITEM>
                <ITEM><NP><NO.P>(b)</NO.P><TXT>Item b.</TXT></NP></ITEM>
            </LIST></P>
        </NP>"#;
        let paras = parse_paragraphs(xml);
        assert_eq!(paras.len(), 1);
        assert_eq!(paras[0].number.as_deref(), Some("1."));
        assert_eq!(paras[0].alineas.len(), 1);
        match &paras[0].alineas[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.intro, "The following apply:");
                assert_eq!(lb.items.len(), 2);
            }
            _ => panic!("expected List alinea"),
        }
    }

    #[test]
    fn p_before_list_becomes_intro() {
        let xml = r#"<P>Items:</P>
                     <LIST TYPE="DASH">
                         <ITEM><NO.P>—</NO.P><P>One.</P></ITEM>
                         <ITEM><NO.P>—</NO.P><P>Two.</P></ITEM>
                     </LIST>"#;
        let paras = parse_paragraphs(xml);
        assert_eq!(paras.len(), 1);
        assert!(paras[0].number.is_none());
        match &paras[0].alineas[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.intro, "Items:");
                assert_eq!(lb.items.len(), 2);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn p_wrapping_list_becomes_list_block() {
        let xml = r#"<P><LIST TYPE="ARAB">
            <ITEM><NP><NO.P>1.</NO.P><TXT>First.</TXT></NP></ITEM>
            <ITEM><NP><NO.P>2.</NO.P><TXT>Second.</TXT></NP></ITEM>
        </LIST></P>"#;
        let paras = parse_paragraphs(xml);
        assert_eq!(paras.len(), 1);
        match &paras[0].alineas[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.items.len(), 2);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn mixed_p_and_np_groups_correctly() {
        // A plain <P> before the first <NP> becomes its own anonymous paragraph.
        let xml = r#"<P>Preamble text.</P>
                     <NP><NO.P>1.</NO.P><TXT>Item one.</TXT></NP>
                     <NP><NO.P>2.</NO.P><TXT>Item two.</TXT></NP>"#;
        let paras = parse_paragraphs(xml);
        assert_eq!(paras.len(), 3);
        assert!(paras[0].number.is_none());
        assert!(matches!(&paras[0].alineas[0],
            Subparagraph::Text { text: t, .. } if t == "Preamble text."));
        assert_eq!(paras[1].number.as_deref(), Some("1."));
        assert_eq!(paras[2].number.as_deref(), Some("2."));
    }
}
