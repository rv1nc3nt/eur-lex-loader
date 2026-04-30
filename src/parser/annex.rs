use roxmltree::Document;

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;
use super::{child, parse_block_children};

/// Parses a Formex annex XML string (`<ANNEX>` root) into an [`Annex`].
///
/// Annex content is heterogeneous: some annexes use plain paragraphs (`<P>`),
/// others use numbered entries (`<NP>` or `<LIST>`), and some group content
/// under titled `<GR.SEQ>` sub-sections. All variants are mapped to
/// [`Subparagraph`] variants via the shared `parse_block_children` utility,
/// the same logic used for article alineas.
///
/// # Errors
///
/// Returns [`crate::error::Error::Xml`] for malformed XML and
/// [`crate::error::Error::MissingElement`] if `<TITLE>` is absent.
pub fn parse_annex(xml: &str) -> Result<Annex, Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();

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

    let content_blocks = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "CONTENTS")
        .map(parse_block_children)
        .unwrap_or_default();

    Ok(Annex { number, subtitle, content_blocks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Subparagraph;

    fn parse_with_contents(contents_inner: &str) -> Vec<Subparagraph> {
        let xml = format!(
            r#"<ANNEX><TITLE><TI><P>ANNEX X</P></TI></TITLE><CONTENTS>{}</CONTENTS></ANNEX>"#,
            contents_inner
        );
        parse_annex(&xml).unwrap().content_blocks
    }

    // ── parse_annex errors ────────────────────────────────────────────────────

    #[test]
    fn parse_annex_missing_title() {
        let result = parse_annex("<ANNEX><CONTENTS/></ANNEX>");
        assert!(matches!(result, Err(crate::error::Error::MissingElement(_))));
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

    // ── content parsing ───────────────────────────────────────────────────────

    #[test]
    fn contents_plain_paragraph() {
        let blocks = parse_with_contents("<P>Some text.</P>");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Subparagraph::Text { text: t, number: None } if t == "Some text."));
    }

    #[test]
    fn contents_empty_paragraph_skipped() {
        let blocks = parse_with_contents("<P>   </P><P>Real text.</P>");
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn contents_np_becomes_numbered_text() {
        let blocks = parse_with_contents("<NP><NO.P>(a)</NO.P><TXT>Body text.</TXT></NP>");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0],
            Subparagraph::Text { number: Some(n), text: t } if n == "(a)" && t == "Body text."));
    }

    #[test]
    fn contents_list_items_with_direct_p() {
        let xml = r#"<LIST TYPE="DASH">
            <ITEM><NO.P>—</NO.P><P>First item.</P></ITEM>
            <ITEM><NO.P>—</NO.P><P>Second item.</P></ITEM>
        </LIST>"#;
        let blocks = parse_with_contents(xml);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.items.len(), 2);
                assert!(matches!(&lb.items[0],
                    Subparagraph::Text { text: t, number: Some(_) } if t == "First item."));
                assert!(matches!(&lb.items[1],
                    Subparagraph::Text { text: t, number: Some(_) } if t == "Second item."));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn contents_list_items_with_np_wrapper() {
        let xml = r#"<LIST TYPE="ARAB">
            <ITEM><NP><NO.P>1.</NO.P><TXT>First description.</TXT></NP></ITEM>
            <ITEM><NP><NO.P>2.</NO.P><TXT>Second description.</TXT></NP></ITEM>
        </LIST>"#;
        let blocks = parse_with_contents(xml);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.items.len(), 2);
                assert!(matches!(&lb.items[0],
                    Subparagraph::Text { number: Some(n), text: t } if n == "1." && t == "First description."));
                assert!(matches!(&lb.items[1],
                    Subparagraph::Text { number: Some(n), text: t } if n == "2." && t == "Second description."));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn p_wrapping_list_expands_to_list() {
        // Annex III style: a <P> wrapping a <LIST> becomes a single List block.
        let xml = r#"<P><LIST TYPE="ARAB">
            <ITEM><NP><NO.P>1.</NO.P><TXT>First.</TXT></NP></ITEM>
            <ITEM><NP><NO.P>2.</NO.P><TXT>Second.</TXT></NP></ITEM>
        </LIST></P>"#;
        let blocks = parse_with_contents(xml);
        assert_eq!(blocks.len(), 1, "expected 1 List block, got {} block(s)", blocks.len());
        match &blocks[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.items.len(), 2);
                assert!(matches!(&lb.items[0],
                    Subparagraph::Text { number: Some(n), text: t } if n == "1." && t == "First."));
                assert!(matches!(&lb.items[1],
                    Subparagraph::Text { number: Some(n), text: t } if n == "2." && t == "Second."));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn list_item_with_nested_list() {
        // Annex III style: a top-level item whose <NP> contains a <P><LIST>.
        // Sub-items appear in the nested List's items.
        let xml = r#"<LIST TYPE="ARAB">
            <ITEM><NP>
                <NO.P>1.</NO.P>
                <TXT>Category:</TXT>
                <P><LIST TYPE="alpha">
                    <ITEM><NP><NO.P>(a)</NO.P><TXT>Sub-item a.</TXT></NP></ITEM>
                    <ITEM><NP><NO.P>(b)</NO.P><TXT>Sub-item b.</TXT></NP></ITEM>
                </LIST></P>
            </NP></ITEM>
            <ITEM><NP><NO.P>2.</NO.P><TXT>No sub-list.</TXT></NP></ITEM>
        </LIST>"#;
        let blocks = parse_with_contents(xml);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Subparagraph::List(lb) => {
                assert_eq!(lb.items.len(), 2);
                match &lb.items[0] {
                    Subparagraph::List(inner) => {
                        assert_eq!(inner.number.as_deref(), Some("1."));
                        assert_eq!(inner.intro, "Category:");
                        assert_eq!(inner.items.len(), 2, "expected 2 sub-items");
                        assert!(matches!(&inner.items[0],
                            Subparagraph::Text { number: Some(n), text: t }
                            if n == "(a)" && t == "Sub-item a."));
                        assert!(matches!(&inner.items[1],
                            Subparagraph::Text { number: Some(n), text: t }
                            if n == "(b)" && t == "Sub-item b."));
                    }
                    _ => panic!("expected nested List for item 1"),
                }
                assert!(matches!(&lb.items[1],
                    Subparagraph::Text { number: Some(n), .. } if n == "2."),
                    "item 2 should be a plain Text");
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn contents_gr_seq_becomes_section() {
        let xml = r#"<GR.SEQ>
            <TITLE><TI><P>Part A</P></TI></TITLE>
            <P>Content paragraph.</P>
        </GR.SEQ>"#;
        let blocks = parse_with_contents(xml);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Subparagraph::Section { title, items } => {
                assert_eq!(title, "Part A");
                assert_eq!(items.len(), 1);
                assert!(matches!(&items[0], Subparagraph::Text { .. }));
            }
            _ => panic!("expected Section"),
        }
    }
}
