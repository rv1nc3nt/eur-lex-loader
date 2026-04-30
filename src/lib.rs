//! Load and parse EUR-Lex Formex XML publications into typed Rust structs.
//!
//! This crate targets EU regulations and directives published in the
//! [Formex 4](https://op.europa.eu/en/web/eu-vocabularies/formex) XML schema
//! by the Publications Office of the European Union. A publication directory
//! contains a registry file (`*.doc.fmx.xml`) that lists the main act file and
//! any annex files; this crate discovers those files, parses them, and
//! assembles a single [`Act`] value.
//!
//! # Entry point
//!
//! [`load_act`] is the primary library API. It takes the path
//! to a Formex publication directory and returns an [`Act`]:
//!
//! ```no_run
//! use std::path::Path;
//! use eur_lex_loader::load_act;
//!
//! let act = load_act(Path::new("/path/to/formex/dir")).unwrap();
//! println!("{}", act.title);
//! for chapter in &act.enacting_terms.chapters {
//!     println!("{}", chapter.title);
//! }
//! ```
//!
//! The companion binary (`eur-lex-loader`) wraps this function and adds a
//! `--celex` flag to fetch a publication directly from the EUR-Lex Cellar API.
//!
//! # Data flow
//!
//! 1. Scan the directory for a `*.doc.fmx.xml` registry file.
//! 2. Parse the registry to discover the act file and annex file paths.
//! 3. Parse the act file into a title, [`model::Preamble`], and
//!    [`model::EnactingTerms`].
//! 4. Parse each annex file into an [`model::Annex`].
//! 5. Assemble everything into an [`Act`].

/// Error types for loading and parsing Formex acts.
pub mod error;
/// File discovery and top-level assembly of an [`Act`].
pub mod loader;
/// Data model: typed structs representing a parsed EU act.
pub mod model;
/// XML parsers for the main act and annex files.
pub mod parser;
/// Plain-text extraction from Formex mixed-content XML nodes.
pub mod text;

pub use loader::load_act;
pub use model::Act;
