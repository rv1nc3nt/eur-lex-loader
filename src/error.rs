#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("XML parse error: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Missing required XML element: <{0}>")]
    MissingElement(&'static str),
}
