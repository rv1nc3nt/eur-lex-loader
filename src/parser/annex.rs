use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;
use super::{child, parse_list};

/// Parses a Formex annex XML string (`<ANNEX>` root) into an [`Annex`].
///
/// Annex content is heterogeneous: some annexes use plain paragraphs (`<P>`),
/// others use numbered entries (`<NP>` or `<LIST>`), and some group content
/// under titled `<GR.SEQ>` sub-sections. All variants are mapped to
/// [`ContentBlock`] variants.
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
        .map(parse_contents)
        .unwrap_or_default();

    Ok(Annex { number, subtitle, content_blocks })
}

/// Iterates the direct children of a `<CONTENTS>` (or `<GR.SEQ>`) element and
/// converts each recognised child into a [`ContentBlock`].
///
/// Unrecognised tags are silently ignored, which makes the function
/// forward-compatible with minor Formex schema variations.
fn parse_contents(node: Node) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "P" => {
                // A <P> sometimes wraps a <LIST> instead of containing inline text
                // (e.g. Annex III). Dispatch to parse_list so items are not
                // flattened into a single paragraph string.
                let lists: Vec<_> = child
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "LIST")
                    .collect();
                if !lists.is_empty() {
                    for list in lists {
                        blocks.extend(parse_list(list));
                    }
                } else {
                    let text = extract_text(child);
                    if !text.is_empty() {
                        blocks.push(ContentBlock::Paragraph(text));
                    }
                }
            }
            "NP" => {
                // Numbered paragraph: <NO.P> label + <TXT> body.
                let number = child
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "NO.P")
                    .map(extract_text)
                    .unwrap_or_default();
                let text = child
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "TXT")
                    .map(extract_text)
                    .unwrap_or_else(|| extract_text(child));
                blocks.push(ContentBlock::ListItem { number, text, sub_items: vec![] });
            }
            "LIST" => {
                blocks.extend(parse_list(child));
            }
            "GR.SEQ" => {
                if let Some(block) = parse_gr_seq(child) {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }
    blocks
}

/// Converts a `<GR.SEQ>` element into a [`ContentBlock::Section`].
///
/// Returns `None` if the element has no `<TITLE><TI>` child (which would make
/// the section unidentifiable).
fn parse_gr_seq(node: Node) -> Option<ContentBlock> {
    let title = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TITLE")?
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TI")
        .map(extract_text)?;

    let blocks = parse_contents(node);
    Some(ContentBlock::Section { title, blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(xml: &str) -> roxmltree::Document<'_> {
        roxmltree::Document::parse(xml).unwrap()
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

    // ── parse_contents ────────────────────────────────────────────────────────

    #[test]
    fn contents_plain_paragraph() {
        let xml = "<CONTENTS><P>Some text.</P></CONTENTS>";
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ContentBlock::Paragraph(t) if t == "Some text."));
    }

    #[test]
    fn contents_empty_paragraph_skipped() {
        let xml = "<CONTENTS><P>   </P><P>Real text.</P></CONTENTS>";
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn contents_np_becomes_list_item() {
        let xml = "<CONTENTS><NP><NO.P>(a)</NO.P><TXT>Body text.</TXT></NP></CONTENTS>";
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::ListItem { number, text, .. } => {
                assert_eq!(number, "(a)");
                assert_eq!(text, "Body text.");
            }
            _ => panic!("expected ListItem"),
        }
    }

    #[test]
    fn contents_list_items_with_direct_p() {
        // Simple items: <NO.P> and <P> are direct children of <ITEM>.
        let xml = r#"<CONTENTS>
            <LIST TYPE="DASH">
                <ITEM><NO.P>—</NO.P><P>First item.</P></ITEM>
                <ITEM><NO.P>—</NO.P><P>Second item.</P></ITEM>
            </LIST>
        </CONTENTS>"#;
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], ContentBlock::ListItem { text, .. } if text == "First item."));
        assert!(matches!(&blocks[1], ContentBlock::ListItem { text, .. } if text == "Second item."));
    }

    #[test]
    fn contents_list_items_with_np_wrapper() {
        // Annex IV style: <ITEM> wraps its content in <NP><NO.P>/<TXT></NP>.
        // The text must not be empty.
        let xml = r#"<CONTENTS>
            <LIST TYPE="ARAB">
                <ITEM><NP><NO.P>1.</NO.P><TXT>First description.</TXT></NP></ITEM>
                <ITEM><NP><NO.P>2.</NO.P><TXT>Second description.</TXT></NP></ITEM>
            </LIST>
        </CONTENTS>"#;
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::ListItem { number, text, .. } => {
                assert_eq!(number, "1.");
                assert_eq!(text, "First description.");
            }
            _ => panic!("expected ListItem"),
        }
        match &blocks[1] {
            ContentBlock::ListItem { number, text, .. } => {
                assert_eq!(number, "2.");
                assert_eq!(text, "Second description.");
            }
            _ => panic!("expected ListItem"),
        }
    }

    #[test]
    fn p_wrapping_list_expands_to_list_items() {
        // Annex III style: a <P> with no text whose only child is a <LIST>.
        // Must yield individual ListItems, not a single flattened Paragraph.
        let xml = r#"<CONTENTS>
            <P><LIST TYPE="ARAB">
                <ITEM><NP><NO.P>1.</NO.P><TXT>First.</TXT></NP></ITEM>
                <ITEM><NP><NO.P>2.</NO.P><TXT>Second.</TXT></NP></ITEM>
            </LIST></P>
        </CONTENTS>"#;
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 2, "expected 2 ListItems, got {} block(s)", blocks.len());
        assert!(matches!(&blocks[0], ContentBlock::ListItem { number, text, .. }
            if number == "1." && text == "First."));
        assert!(matches!(&blocks[1], ContentBlock::ListItem { number, text, .. }
            if number == "2." && text == "Second."));
    }

    #[test]
    fn list_item_with_nested_list() {
        // Annex III style: a top-level item whose <NP> contains a <P><LIST>.
        // Sub-items must appear in sub_items, not be discarded.
        let xml = r#"<CONTENTS>
            <LIST TYPE="ARAB">
                <ITEM><NP>
                    <NO.P>1.</NO.P>
                    <TXT>Category:</TXT>
                    <P><LIST TYPE="alpha">
                        <ITEM><NP><NO.P>(a)</NO.P><TXT>Sub-item a.</TXT></NP></ITEM>
                        <ITEM><NP><NO.P>(b)</NO.P><TXT>Sub-item b.</TXT></NP></ITEM>
                    </LIST></P>
                </NP></ITEM>
                <ITEM><NP><NO.P>2.</NO.P><TXT>No sub-list.</TXT></NP></ITEM>
            </LIST>
        </CONTENTS>"#;
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 2);
        match &blocks[0] {
            ContentBlock::ListItem { number, text, sub_items } => {
                assert_eq!(number, "1.");
                assert_eq!(text, "Category:");
                assert_eq!(sub_items.len(), 2, "expected 2 sub-items");
                assert!(matches!(&sub_items[0],
                    ContentBlock::ListItem { number, text, .. }
                    if number == "(a)" && text == "Sub-item a."));
                assert!(matches!(&sub_items[1],
                    ContentBlock::ListItem { number, text, .. }
                    if number == "(b)" && text == "Sub-item b."));
            }
            _ => panic!("expected ListItem"),
        }
        match &blocks[1] {
            ContentBlock::ListItem { sub_items, .. } => {
                assert!(sub_items.is_empty(), "item 2 should have no sub-items");
            }
            _ => panic!("expected ListItem"),
        }
    }

    #[test]
    fn contents_gr_seq_becomes_section() {
        let xml = r#"<CONTENTS>
            <GR.SEQ>
                <TITLE><TI><P>Part A</P></TI></TITLE>
                <P>Content paragraph.</P>
            </GR.SEQ>
        </CONTENTS>"#;
        let d = doc(xml);
        let blocks = parse_contents(d.root_element());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Section { title, blocks: inner } => {
                assert_eq!(title, "Part A");
                assert_eq!(inner.len(), 1);
            }
            _ => panic!("expected Section"),
        }
    }
}
