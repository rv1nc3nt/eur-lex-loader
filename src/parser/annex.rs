use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;

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

    let title_node = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TITLE")
        .ok_or(Error::MissingElement("TITLE"))?;

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
                let text = extract_text(child);
                // Skip elements that are empty after whitespace normalisation
                // (e.g. page-break processing instructions with no text).
                if !text.is_empty() {
                    blocks.push(ContentBlock::Paragraph(text));
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
                blocks.push(ContentBlock::ListItem { number, text });
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

/// Converts a `<LIST>` element into a sequence of [`ContentBlock::ListItem`]s,
/// one per `<ITEM>` child.
fn parse_list(node: Node) -> Vec<ContentBlock> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ITEM")
        .map(|item| {
            let number = item
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "NO.P")
                .map(extract_text)
                .unwrap_or_default();
            // An item may have multiple <P> children; join them with a space.
            let text = item
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "P")
                .map(extract_text)
                .collect::<Vec<_>>()
                .join(" ");
            ContentBlock::ListItem { number, text }
        })
        .collect()
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
            ContentBlock::ListItem { number, text } => {
                assert_eq!(number, "(a)");
                assert_eq!(text, "Body text.");
            }
            _ => panic!("expected ListItem"),
        }
    }

    #[test]
    fn contents_list_items() {
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
