/// Errors that can occur while loading or parsing a Formex act.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A `roxmltree` XML parse failure (malformed or invalid XML).
    #[error("XML parse error: {0}")]
    Xml(#[from] roxmltree::Error),

    /// A filesystem I/O failure when reading a `.fmx.xml` file.
    #[error("I/O error reading {path}: {source}")]
    Io {
        /// Path of the file that could not be read.
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// A required XML element was absent from the document.
    ///
    /// The string is a human-readable XPath-style description of the expected
    /// element, e.g. `"ENACTING.TERMS"` or `"DOC.MAIN.PUB/REF.PHYS[@FILE]"`.
    #[error("Missing required XML element: <{0}>")]
    MissingElement(&'static str),
}
