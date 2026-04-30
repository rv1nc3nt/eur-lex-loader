//! XML parsers for Formex regulation files.
//!
//! Public API: [`parse_act`](crate::parser::parse_act) and [`parse_annex`](crate::parser::parse_annex) each parse one `.fmx.xml` file
//! into the corresponding model type. Two `pub(crate)` utilities are shared by
//! both parsers:
//!
//! - `child` — looks up a mandatory direct child element by tag name.
//! - `parse_list` — converts a `<LIST>` element into [`crate::model::ContentBlock::ListItem`]s.

/// Parser for the main act XML file (`<ACT>` root).
mod act;
/// Parser for annex XML files (`<ANNEX>` root).
mod annex;

pub use act::parse_act;
pub use annex::parse_annex;

use roxmltree::Node;

use crate::error::Error;
use crate::model::{ContentBlock, ListBlock, Subparagraph};
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

/// Converts a `<LIST>` element into a sequence of [`ContentBlock::ListItem`]s.
///
/// Used exclusively by the annex parser. For article paragraphs use
/// [`parse_list_as_subparagraphs`] instead.
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

/// Converts a `<LIST>` element into a sequence of [`Subparagraph`]s for use
/// inside article [`crate::model::Paragraph`]s.
///
/// - Simple item (no nested list): → [`Subparagraph::Text`]`{ text, number: Some(number) }`
/// - NP-wrapped item with a nested `<LIST>`: → [`Subparagraph::List`] carrying
///   the item number, intro text, and recursively parsed sub-items.
pub(crate) fn parse_list_as_subparagraphs(node: Node) -> Vec<Subparagraph> {
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
                let nested: Vec<Subparagraph> = np
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "P")
                    .flat_map(|p| {
                        p.children()
                            .filter(|n| n.is_element() && n.tag_name().name() == "LIST")
                            .flat_map(parse_list_as_subparagraphs)
                    })
                    .collect();
                if nested.is_empty() {
                    Subparagraph::Text { text, number: Some(number) }
                } else {
                    Subparagraph::List(ListBlock { number: Some(number), intro: text, items: nested })
                }
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
                Subparagraph::Text { text, number: Some(number) }
            }
        })
        .collect()
}
