//! XML parsers for Formex act files.
//!
//! Public API: [`parse_act`](crate::parser::parse_act) and [`parse_annex`](crate::parser::parse_annex) each parse one `.fmx.xml` file
//! into the corresponding model type. Several `pub(crate)` utilities are shared by
//! both parsers:
//!
//! - `child` — looks up a mandatory direct child element by tag name.
//! - `parse_block_children` — converts the direct children of any block-level
//!   element into [`crate::model::Subparagraph`]s, handling `<P>`, `<LIST>`,
//!   `<NP>`, `<GR.TBL>`, and `<TBL>`.
//! - `parse_single_tbl` — converts one `<TBL>` element into a `Subparagraph::Table`.
//! - `parse_table` — converts a `<GR.TBL>` wrapper into one `Table` per `<TBL>` child.

/// Parser for the main act XML file (`<ACT>` root).
mod act;
/// Parser for annex XML files (`<ANNEX>` root).
mod annex;

pub use act::{parse_regular_act, parse_consolidated_act};
pub use annex::{parse_annex, parse_cons_annex};

use roxmltree::Node;

use crate::error::Error;
use crate::model::{Cell, ListBlock, Row, Subparagraph, Table};
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

/// Converts a `<LIST>` element into a sequence of [`Subparagraph`]s.
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

/// Converts a single `<TBL>` element into a [`Subparagraph::Table`].
pub(crate) fn parse_single_tbl(tbl: Node) -> Subparagraph {
    let col_count = tbl.attribute("COLS").and_then(|v| v.parse().ok()).unwrap_or(0);
    let title = tbl
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "TITLE")
        .and_then(|t| t.children().find(|n| n.is_element() && n.tag_name().name() == "TI"))
        .map(extract_text)
        .filter(|s| !s.is_empty());
    let rows: Vec<Row> = tbl
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "CORPUS")
        .into_iter()
        .flat_map(|corpus| {
            // CORPUS contains ROW and BLK elements; BLK groups rows (e.g. REACH).
            corpus.children().filter(|n| n.is_element()).flat_map(|n| {
                match n.tag_name().name() {
                    "ROW" => vec![n],
                    "BLK" => n.children()
                        .filter(|c| c.is_element() && c.tag_name().name() == "ROW")
                        .collect(),
                    _ => vec![],
                }
            })
        })
        .map(|row| {
            let is_header = row.attribute("TYPE") == Some("HEADER");
            let cells: Vec<Cell> = row
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "CELL")
                .map(|cell| Cell {
                    text: extract_text(cell),
                    is_header: cell.attribute("TYPE") == Some("HEADER"),
                })
                .collect();
            let cell_count = cells.len();
            Row { cells, cell_count, is_header }
        })
        .collect();
    let row_count = rows.len();
    Subparagraph::Table(Table { col_count, title, rows, row_count })
}

/// Converts a `<GR.TBL>` element into one [`Subparagraph::Table`] per `<TBL>` child.
pub(crate) fn parse_table(gr_tbl: Node) -> Vec<Subparagraph> {
    gr_tbl
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "TBL")
        .map(parse_single_tbl)
        .collect()
}

/// Converts the direct children of a block-level element into [`Subparagraph`]s.
///
/// Handles Formex block elements that appear in article alineas:
///
/// | XML element | Output |
/// |---|---|
/// | `<P>` (plain) | pending intro; flushed as `Text` if not followed by `<LIST>` |
/// | `<P>` immediately before sibling `<LIST>` | grouped into `List` with intro |
/// | `<P>` wrapping `<LIST>` or `<TBL>` children | those blocks expanded directly |
/// | `<LIST>` | `List` via [`parse_list_as_subparagraphs`] |
/// | `<GR.TBL>` | one `Table` per `<TBL>` child |
/// | `<TBL>` | `Table` via [`parse_single_tbl`] |
/// | `<NP>` | `Text { number: Some(NO.P), text: TXT }` |
/// | `<TITLE>` | skipped (structural, extracted by callers) |
/// | other | text content as `Text` |
pub(crate) fn parse_block_children(node: Node) -> Vec<Subparagraph> {
    let mut result: Vec<Subparagraph> = Vec::new();
    let mut pending: Option<String> = None;

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "P" => {
                // Check if this <P> directly wraps <LIST> or <TBL> children
                // (e.g. Annex III of the EU AI Act; REACH annexes).
                // If so, expand those blocks directly with no intro.
                let nested_blocks: Vec<_> = child
                    .children()
                    .filter(|n| {
                        n.is_element()
                            && matches!(n.tag_name().name(), "LIST" | "TBL")
                    })
                    .collect();
                if !nested_blocks.is_empty() {
                    if let Some(t) = pending.take() {
                        result.push(Subparagraph::Text { text: t, number: None });
                    }
                    for block in nested_blocks {
                        match block.tag_name().name() {
                            "LIST" => result.push(Subparagraph::List(ListBlock {
                                number: None,
                                intro: String::new(),
                                items: parse_list_as_subparagraphs(block),
                            })),
                            _ => result.push(parse_single_tbl(block)),
                        }
                    }
                } else {
                    if let Some(t) = pending.take() {
                        result.push(Subparagraph::Text { text: t, number: None });
                    }
                    let t = extract_text(child);
                    if !t.is_empty() {
                        pending = Some(t);
                    }
                }
            }
            "LIST" => {
                let intro = pending.take().unwrap_or_default();
                result.push(Subparagraph::List(ListBlock {
                    number: None,
                    intro,
                    items: parse_list_as_subparagraphs(child),
                }));
            }
            "NP" => {
                if let Some(t) = pending.take() {
                    result.push(Subparagraph::Text { text: t, number: None });
                }
                let number = child
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "NO.P")
                    .map(extract_text)
                    .unwrap_or_default();
                // <TXT> is the standard body; fall back to the full <NP> text when absent.
                let text = child
                    .children()
                    .find(|n| n.is_element() && n.tag_name().name() == "TXT")
                    .map(extract_text)
                    .unwrap_or_else(|| extract_text(child));
                result.push(Subparagraph::Text { text, number: Some(number) });
            }
            "TITLE" => {
                // Structural title elements are extracted by callers; skip here.
            }
            "GR.TBL" => {
                if let Some(t) = pending.take() {
                    result.push(Subparagraph::Text { text: t, number: None });
                }
                result.extend(parse_table(child));
            }
            "TBL" => {
                if let Some(t) = pending.take() {
                    result.push(Subparagraph::Text { text: t, number: None });
                }
                result.push(parse_single_tbl(child));
            }
            _ => {
                // Unrecognised block elements (e.g. <TABLE>, <FORMULA>) are
                // reduced to their text content. Structure is lost but no text
                // is silently dropped.
                if let Some(t) = pending.take() {
                    result.push(Subparagraph::Text { text: t, number: None });
                }
                let t = extract_text(child);
                if !t.is_empty() {
                    pending = Some(t);
                }
            }
        }
    }
    // Flush any trailing <P> not followed by a <LIST>.
    if let Some(t) = pending {
        result.push(Subparagraph::Text { text: t, number: None });
    }
    // Pure inline node — wrap the whole text as a single Text block.
    if result.is_empty() {
        let t = extract_text(node);
        if !t.is_empty() {
            result.push(Subparagraph::Text { text: t, number: None });
        }
    }
    result
}
