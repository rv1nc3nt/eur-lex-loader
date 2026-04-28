//! XML parsers for Formex regulation files.
//!
//! Public API: [`parse_act`] and [`parse_annex`] each parse one `.fmx.xml` file
//! into the corresponding model type. Two `pub(crate)` utilities are shared by
//! both parsers:
//!
//! - [`child`] — looks up a mandatory direct child element by tag name.
//! - [`parse_list`] — converts a `<LIST>` element into [`ContentBlock::ListItem`]s.

/// Parser for the main act XML file (`<ACT>` root).
mod act;
/// Parser for annex XML files (`<ANNEX>` root).
mod annex;

pub use act::parse_act;
pub use annex::parse_annex;

use roxmltree::Node;

use crate::error::Error;
use crate::model::ContentBlock;
use crate::text::extract_text;

/// Returns the first direct child element of `node` with the given tag name.
///
/// # Errors
///
/// Returns [`Error::MissingElement`] when no matching child is found.
pub(crate) fn child<'a>(node: Node<'a, 'a>, tag: &'static str) -> Result<Node<'a, 'a>, Error> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
        .ok_or(Error::MissingElement(tag))
}

/// Converts a `<LIST>` element into a sequence of [`ContentBlock::ListItem`]s,
/// one per `<ITEM>` child.
///
/// Two item structures appear in Formex documents:
/// - Simple: `<ITEM><NO.P>—</NO.P><P>text</P></ITEM>`
/// - NP-wrapped: `<ITEM><NP><NO.P>1.</NO.P><TXT>text</TXT></NP></ITEM>`
///
/// In the NP-wrapped form a `<P>` inside the `<NP>` may itself wrap a nested
/// `<LIST>`, whose items are collected into `sub_items`.
pub(crate) fn parse_list(node: Node) -> Vec<ContentBlock> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ITEM")
        .map(|item| {
            if let Some(np) = item
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "NP")
            {
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
                let sub_items: Vec<ContentBlock> = np
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "P")
                    .flat_map(|p| {
                        p.children()
                            .filter(|n| n.is_element() && n.tag_name().name() == "LIST")
                            .flat_map(parse_list)
                    })
                    .collect();
                ContentBlock::ListItem { number, text, sub_items }
            } else {
                let number = item
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "NO.P")
                    .map(extract_text)
                    .unwrap_or_default();
                let text = item
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "P")
                    .map(extract_text)
                    .collect::<Vec<_>>()
                    .join(" ");
                ContentBlock::ListItem { number, text, sub_items: vec![] }
            }
        })
        .collect()
}
