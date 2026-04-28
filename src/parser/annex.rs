use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;

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
        .map(|contents| parse_contents(contents))
        .unwrap_or_default();

    Ok(Annex { number, subtitle, content_blocks })
}

fn parse_contents(node: Node) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "P" => {
                let text = extract_text(child);
                if !text.is_empty() {
                    blocks.push(ContentBlock::Paragraph(text));
                }
            }
            "NP" => {
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

fn parse_list(node: Node) -> Vec<ContentBlock> {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ITEM")
        .map(|item| {
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
            ContentBlock::ListItem { number, text }
        })
        .collect()
}

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
