use std::fs;
use std::path::Path;

use roxmltree::Document;

use crate::error::Error;
use crate::model::Regulation;
use crate::parser::{parse_act, parse_annex};

pub fn load_regulation(data_dir: &Path) -> Result<Regulation, Error> {
    let doc_file = find_doc_file(data_dir)?;
    let (main_file, annex_files) = discover_files(&doc_file)?;

    let main_xml = read_file(&data_dir.join(&main_file))?;
    let (title, preamble, enacting_terms) = parse_act(&main_xml)?;

    let annexes = annex_files
        .iter()
        .map(|f| {
            let xml = read_file(&data_dir.join(f))?;
            parse_annex(&xml)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Regulation { title, preamble, enacting_terms, annexes })
}

fn find_doc_file(data_dir: &Path) -> Result<std::path::PathBuf, Error> {
    let entries = fs::read_dir(data_dir).map_err(|e| Error::Io {
        path: data_dir.display().to_string(),
        source: e,
    })?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".doc.fmx.xml") {
            return Ok(entry.path());
        }
    }

    Err(Error::MissingElement("*.doc.fmx.xml"))
}

fn discover_files(doc_file: &Path) -> Result<(String, Vec<String>), Error> {
    let xml = read_file(doc_file)?;
    let document = Document::parse(&xml)?;
    let root = document.root_element();

    let fmx = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "FMX")
        .ok_or(Error::MissingElement("FMX"))?;

    let main_file = fmx
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "DOC.MAIN.PUB")
        .and_then(|n| {
            n.children()
                .find(|c| c.is_element() && c.tag_name().name() == "REF.PHYS")
        })
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

    Ok((main_file, annex_files))
}

fn read_file(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|e| Error::Io {
        path: path.display().to_string(),
        source: e,
    })
}
