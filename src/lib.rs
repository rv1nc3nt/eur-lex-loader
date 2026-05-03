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
//! use eur_lex_loader::{load_act, Act};
//!
//! let act = load_act(Path::new("/path/to/formex/dir")).unwrap();
//!
//! // Convenience methods work on both Regular and Consolidated variants:
//! println!("{}", act.title());
//! if let Some(celex) = &act.metadata().celex {
//!     println!("CELEX: {celex}");
//! }
//! for chapter in &act.enacting_terms().chapters {
//!     println!("{}", chapter.title);
//! }
//! if let Some(def) = act.definitions().get("AI system") {
//!     println!("{def}");
//! }
//!
//! // Pattern-match to access variant-specific fields (e.g. preamble visas):
//! if let Act::Regular(reg) = &act {
//!     println!("{} visas", reg.preamble.visas.len());
//! }
//! ```
//!
//! The companion binary (`eur-lex-loader`) wraps this function and adds a
//! `--celex` flag to fetch a publication directly from the EUR-Lex Cellar API.
//!
//! # Data flow
//!
//! 1. Scan the directory for a `*.doc.fmx.xml` registry file.
//! 2. Parse the registry to extract bibliographic [`Metadata`] (CELEX number,
//!    document date, legal value, language, authors, Official Journal reference,
//!    page range) and discover the act file and annex file paths.
//! 3. For regular acts, parse the main file into a [`model::Preamble`] and
//!    [`model::EnactingTerms`], then parse each separate annex file.
//! 4. For consolidated acts, parse the single file into a
//!    [`model::ConsolidatedPreamble`] and [`model::EnactingTerms`], then
//!    extract inline `<CONS.ANNEX>` elements.
//! 5. Extract definitions from any article titled "Definitions" into a
//!    [`std::collections::HashMap`].
//! 6. Assemble everything into an [`Act::Regular`] or [`Act::Consolidated`].

/// Error types for loading and parsing Formex acts.
pub mod error;
/// File discovery and top-level assembly of an [`Act`].
pub mod loader;
/// Data model: typed structs representing a parsed EU act.
pub mod model;
/// XML parsers for the main act and annex files.
pub mod parser;

pub use loader::load_act;
pub use model::{Act, RegularAct, ConsolidatedAct, ConsolidatedPreamble, Item, ItemContent, Metadata, OfficialJournal};
