use roxmltree::{Document, Node};

use crate::error::Error;
use crate::model::*;
use crate::text::extract_text;

pub fn parse_act(xml: &str) -> Result<(String, Preamble, EnactingTerms), Error> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();

    let title = parse_title(child(root, "TITLE")?)?;
    let preamble = parse_preamble(child(root, "PREAMBLE")?)?;
    let enacting_terms = parse_enacting_terms(child(root, "ENACTING.TERMS")?)?;

    Ok((title, preamble, enacting_terms))
}

fn child<'a>(node: Node<'a, 'a>, tag: &'static str) -> Result<Node<'a, 'a>, Error> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
        .ok_or(Error::MissingElement(tag))
}

fn parse_title(node: Node) -> Result<String, Error> {
    let ti = child(node, "TI")?;
    let parts: Vec<String> = ti
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "P")
        .map(extract_text)
        .collect();
    Ok(parts.join(" "))
}

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

fn parse_enacting_terms(node: Node) -> Result<EnactingTerms, Error> {
    let chapters = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "DIVISION")
        .map(parse_chapter)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EnactingTerms { chapters })
}

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
        let alineas: Vec<String> = node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "ALINEA")
            .map(extract_text)
            .collect();
        vec![Paragraph { number: None, alineas }]
    };

    Ok(Article { number, title, paragraphs })
}

fn parse_paragraph(node: Node) -> Result<Paragraph, Error> {
    let number = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "NO.PARAG")
        .map(extract_text);

    let alineas: Vec<String> = node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "ALINEA")
        .map(extract_text)
        .collect();

    Ok(Paragraph { number, alineas })
}
