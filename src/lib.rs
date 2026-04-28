/// Error types for loading and parsing Formex regulations.
pub mod error;
/// File discovery and top-level assembly of a [`model::Regulation`].
pub mod loader;
/// Data model: typed structs representing a parsed EU regulation.
pub mod model;
/// XML parsers for the main act and annex files.
pub mod parser;
/// Plain-text extraction from Formex mixed-content XML nodes.
pub mod text;
