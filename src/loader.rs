//! File discovery and assembly of a complete [`crate::model::Act`] from a
//! Formex publication directory.
//!
//! The entry point is [`load_act`], which locates the `*.doc.xml` registry,
//! parses bibliographic metadata and the ordered file list, reads each Formex
//! file, and delegates to the [`crate::parser`] module to produce typed structs.

use std::fs;
use std::path::Path;

use roxmltree::Document;

use std::collections::HashMap;

use crate::error::Error;
use crate::model::{Act, ConsolidatedAct, Metadata, OfficialJournal, RegularAct, ChapterContents, ListBlock, Subparagraph};
use crate::parser::{parse_regular_act, parse_consolidated_act, parse_annex, parse_cons_annex};

/// Loads a complete act from a directory of Formex `.fmx.xml` files.
///
/// The directory must contain exactly one `*.doc.fmx.xml` registry file.
/// That registry is parsed first to discover the canonical filename of the
/// main act and the ordered list of annex filenames; all files are then read
/// from the same directory.
///
/// # Errors
///
/// Returns [`Error::Io`] if the directory cannot be read or a file is missing,
/// [`Error::Xml`] if any file contains malformed XML, and
/// [`Error::MissingElement`] if a required Formex element is absent.
pub fn load_act(data_dir: &Path) -> Result<Act, Error> {
    let doc_file = find_doc_file(data_dir)?;
    let (metadata, main_file, annex_files) = parse_doc_file(&doc_file)?;

    let main_xml = read_file(&data_dir.join(&main_file))?;

    if is_consolidated(&main_xml) {
        let (title, preamble, enacting_terms) = parse_consolidated_act(&main_xml)?;
        let annexes = parse_cons_annex(&main_xml)?;
        let definitions = extract_definitions(&enacting_terms);
        Ok(Act::Consolidated(ConsolidatedAct { metadata, title, preamble, enacting_terms, annexes, definitions }))
    } else {
        let (title, preamble, enacting_terms) = parse_regular_act(&main_xml)?;
        let annexes = annex_files
            .iter()
            .map(|f| {
                let xml = read_file(&data_dir.join(f))?;
                parse_annex(&xml)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let definitions = extract_definitions(&enacting_terms);
        Ok(Act::Regular(RegularAct { metadata, title, preamble, enacting_terms, annexes, definitions }))
    }
}

/// Traverses `enacting_terms` to find all articles whose title contains
/// "Definitions" and extracts a term → definition-text map from their list items.
fn extract_definitions(enacting_terms: &crate::model::EnactingTerms) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut articles = enacting_terms.chapters.iter().flat_map(|ch| match &ch.contents {
        ChapterContents::Articles(arts) => arts.iter().collect::<Vec<_>>(),
        ChapterContents::Sections(secs) => {
            secs.iter().flat_map(|s| s.articles.iter()).collect()
        }
    });
    if let Some(article) = articles.find(|a| a.title.as_deref() == Some("Definitions")) {
        for para in &article.paragraphs {
            for alinea in &para.alineas {
                collect_definition_items(alinea, &mut map);
            }
        }
    }
    map
}

/// Recursively visits a [`Subparagraph`] and inserts any definition it finds
/// into `map`. A definition is recognised by a leading `\u{201C}term\u{201D}`
/// pair produced by Formex `<QUOT.START>` / `<QUOT.END>` markers.
fn collect_definition_items(sub: &Subparagraph, map: &mut HashMap<String, String>) {
    match sub {
        Subparagraph::List(ListBlock { intro, items, .. }) => {
            // Items with sub-lists carry their definition in the intro text.
            if let Some(term) = extract_term(intro) {
                map.insert(term.to_owned(), intro.clone());
            }
            for item in items {
                collect_definition_items(item, map);
            }
        }
        Subparagraph::Text { text, .. } => {
            if let Some(term) = extract_term(text) {
                map.insert(term.to_owned(), text.clone());
            }
        }
        Subparagraph::Table(_) => {}
    }
}

/// Returns the defined term from a definition text, i.e. the substring between
/// the first `\u{201C}` (left double quotation mark) and the first `\u{201D}`
/// (right double quotation mark). Returns `None` if no such pair exists.
fn extract_term(text: &str) -> Option<&str> {
    let start = text.find('\u{201C}')? + '\u{201C}'.len_utf8();
    let end = text[start..].find('\u{201D}')?;
    Some(&text[start..start + end])
}

/// Scans `data_dir` for the `*.doc.fmx.xml` or `*.doc.xml` registry file.
fn find_doc_file(data_dir: &Path) -> Result<std::path::PathBuf, Error> {
    let entries = fs::read_dir(data_dir).map_err(|e| Error::Io {
        path: data_dir.display().to_string(),
        source: e,
    })?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".doc.fmx.xml") || name.ends_with(".doc.xml") {
            return Ok(entry.path());
        }
    }

    Err(Error::MissingElement("*.doc.fmx.xml or *.doc.xml"))
}

/// Parses a `.doc.xml` registry to obtain the file list and bibliographic metadata.
///
/// Returns `(metadata, main_act_filename, vec_of_annex_filenames)`. The annex
/// list preserves the `NO.SEQ` order declared in the registry rather than
/// relying on filesystem sort order.
fn parse_doc_file(doc_file: &Path) -> Result<(Metadata, String, Vec<String>), Error> {
    let xml = read_file(doc_file)?;
    let document = Document::parse(&xml)?;
    let root = document.root_element();

    let fmx = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "FMX")
        .ok_or(Error::MissingElement("FMX"))?;

    // ── file discovery ────────────────────────────────────────────────────────
    let doc_main = fmx
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "DOC.MAIN.PUB")
        .ok_or(Error::MissingElement("DOC.MAIN.PUB"))?;

    let main_file = doc_main
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "REF.PHYS")
        .and_then(|n| n.attribute("FILE"))
        .ok_or(Error::MissingElement("DOC.MAIN.PUB/REF.PHYS[@FILE]"))?
        .to_string();

    let annex_files = fmx
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "DOC.SUB.PUB")
        .filter_map(|n| {
            n.children()
                .find(|c| c.is_element() && c.tag_name().name() == "REF.PHYS")
                .and_then(|c| c.attribute("FILE"))
                .map(|s| s.to_string())
        })
        .collect();

    // ── BIB.DOC ───────────────────────────────────────────────────────────────
    let mut prod_id: Option<String> = None;
    let mut fin_id: Option<String> = None;
    let mut authors: Vec<String> = Vec::new();
    let mut eea_relevant = false;

    if let Some(bib) = fmx.children().find(|n| n.is_element() && n.tag_name().name() == "BIB.DOC") {
        for child in bib.children().filter(|n| n.is_element()) {
            match child.tag_name().name() {
                "PROD.ID" => prod_id = Some(child.text().unwrap_or_default().to_string()),
                "FIN.ID"  => fin_id  = Some(child.text().unwrap_or_default().to_string()),
                "AUTHOR"  => { authors.push(child.text().unwrap_or_default().to_string()); }
                "EEA"     => eea_relevant = true,
                _         => {}
            }
        }
    }

    // ── PUBLICATION.REF ───────────────────────────────────────────────────────
    let official_journal = fmx
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "PUBLICATION.REF")
        .map(|pub_ref| {
            let mut collection = String::new();
            let mut number = String::new();
            let mut date = String::new();
            let mut language = String::new();
            for child in pub_ref.children().filter(|n| n.is_element()) {
                match child.tag_name().name() {
                    "COLL"  => collection = child.text().unwrap_or_default().to_string(),
                    "NO.OJ" => number     = child.text().unwrap_or_default().to_string(),
                    "DATE"  => date       = child.attribute("ISO").unwrap_or_default().to_string(),
                    "LG.OJ" => language   = child.text().unwrap_or_default().to_string(),
                    _ => {}
                }
            }
            OfficialJournal { collection, number, date, language }
        });

    // ── DOC.MAIN.PUB metadata ─────────────────────────────────────────────────
    let mut celex: Option<String> = None;
    let mut document_date: Option<String> = None;
    let mut legal_value: Option<String> = None;
    let mut language: Option<String> = None;
    let mut page_first: Option<u32> = None;
    let mut page_last: Option<u32> = None;
    let mut page_total: Option<u32> = None;

    for child in doc_main.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "NO.CELEX"    => celex        = Some(child.text().unwrap_or_default().to_string()),
            "DATE"        => document_date = Some(child.attribute("ISO").unwrap_or_default().to_string()),
            "LEGAL.VALUE" => legal_value  = Some(child.text().unwrap_or_default().to_string()),
            "LG.DOC"      => language     = Some(child.text().unwrap_or_default().to_string()),
            "PAGE.FIRST"  => page_first   = child.text().and_then(|t| t.parse().ok()),
            "PAGE.LAST"   => page_last    = child.text().and_then(|t| t.parse().ok()),
            "PAGE.TOTAL"  => page_total   = child.text().and_then(|t| t.parse().ok()),
            _ => {}
        }
    }

    let metadata = Metadata {
        celex,
        document_date,
        legal_value,
        language,
        authors,
        eea_relevant,
        official_journal,
        page_first,
        page_last,
        page_total,
        prod_id,
        fin_id,
    };

    Ok((metadata, main_file, annex_files))
}

/// Returns `true` when the XML document uses `<CONS.ACT>` as its root element,
/// indicating a consolidated act where annexes are embedded inline.
fn is_consolidated(xml: &str) -> bool {
    roxmltree::Document::parse(xml)
        .map(|d| d.root_element().tag_name().name() == "CONS.ACT")
        .unwrap_or(false)
}

/// Reads a file to a `String`, wrapping any I/O error with the file path.
fn read_file(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|e| Error::Io {
        path: path.display().to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::extract_term;

    #[test]
    /// A string opening and closing with Unicode typographic quotes returns the text between them.
    fn extract_term_basic() {
        let text = "\u{201C}AI system\u{201D} means a machine-based system";
        assert_eq!(extract_term(text), Some("AI system"));
    }

    #[test]
    /// A string with no opening quote returns `None`.
    fn extract_term_no_quotes_returns_none() {
        assert_eq!(extract_term("plain text without quotes"), None);
    }

    #[test]
    /// A string with an opening quote but no closing quote returns `None`.
    fn extract_term_only_opening_quote_returns_none() {
        assert_eq!(extract_term("\u{201C}unclosed term"), None);
    }
}
