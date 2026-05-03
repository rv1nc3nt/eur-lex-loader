use std::collections::HashMap;

use serde::{Deserialize, Serialize}; // Deserialize needed for Subparagraph/ListBlock in tests

/// A parsed EU legislative act.
///
/// The two variants reflect the two Formex publication formats:
/// - [`Act::Regular`] — an original act (`<ACT>` root) with a full preamble
///   including legal-basis citations (visas) and numbered recitals.
/// - [`Act::Consolidated`] — a consolidated version (`<CONS.ACT>` root) with a
///   slim preamble: visas and recitals are structurally absent.
///
/// Serialises without a variant tag (`#[serde(untagged)]`), so the JSON output
/// is structurally identical to the underlying struct for each variant.
///
/// Convenience methods ([`Act::title`], [`Act::enacting_terms`],
/// [`Act::annexes`], [`Act::definitions`]) provide access to shared fields
/// without pattern-matching on the variant.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Act {
    /// An original act with a full preamble.
    Regular(RegularAct),
    /// A consolidated act with a slim preamble (no visas or recitals).
    Consolidated(ConsolidatedAct),
}

impl Act {
    /// The full title of the act.
    pub fn title(&self) -> &str {
        match self {
            Act::Regular(a) => &a.title,
            Act::Consolidated(a) => &a.title,
        }
    }

    /// The operative body of the act.
    pub fn enacting_terms(&self) -> &EnactingTerms {
        match self {
            Act::Regular(a) => &a.enacting_terms,
            Act::Consolidated(a) => &a.enacting_terms,
        }
    }

    /// The annexes, in document order.
    pub fn annexes(&self) -> &[Annex] {
        match self {
            Act::Regular(a) => &a.annexes,
            Act::Consolidated(a) => &a.annexes,
        }
    }

    /// Definitions extracted from any "Definitions" article. Empty when absent.
    pub fn definitions(&self) -> &HashMap<String, String> {
        match self {
            Act::Regular(a) => &a.definitions,
            Act::Consolidated(a) => &a.definitions,
        }
    }
}

/// A complete original EU act (`<ACT>` root), with a full preamble.
#[derive(Serialize)]
pub struct RegularAct {
    /// The full title of the act, e.g. `"Regulation (EU) 2024/1689 …"`.
    pub title: String,
    /// The preamble: opening formula, legal bases, numbered recitals, enacting formula.
    pub preamble: Preamble,
    /// The operative body of the act.
    pub enacting_terms: EnactingTerms,
    /// The annexes, in the order declared by the `.doc.fmx.xml` registry.
    pub annexes: Vec<Annex>,
    /// Definitions extracted from any "Definitions" article. Omitted from JSON when absent.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub definitions: HashMap<String, String>,
}

/// A complete consolidated EU act (`<CONS.ACT>` root), with a slim preamble.
///
/// Consolidated acts do not carry visas or recitals; use [`RegularAct`] when
/// you need those fields.
#[derive(Serialize)]
pub struct ConsolidatedAct {
    /// The full title of the act.
    pub title: String,
    /// The slim preamble: opening formula and enacting formula only.
    pub preamble: ConsolidatedPreamble,
    /// The operative body of the act.
    pub enacting_terms: EnactingTerms,
    /// The annexes, parsed inline from `<CONS.ANNEX>` elements.
    pub annexes: Vec<Annex>,
    /// Definitions extracted from any "Definitions" article. Omitted from JSON when absent.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub definitions: HashMap<String, String>,
}

/// The slim preamble of a consolidated act.
///
/// Consolidated acts (`<CONS.ACT>`) carry only the opening institutional
/// formula and the enacting formula; legal-basis citations and recitals are
/// structurally absent (unlike [`Preamble`] where they are optional lists).
#[derive(Serialize)]
pub struct ConsolidatedPreamble {
    /// Opening institutional formula (`<PREAMBLE.INIT>`).
    pub init: String,
    /// Closing enacting formula (`<PREAMBLE.FINAL>`).
    pub enacting_formula: String,
}

/// The preamble of an act (`<PREAMBLE>`).
///
/// Formex splits the preamble into four structural parts: the opening
/// institutional formula (`PREAMBLE.INIT`), the legal bases (`GR.VISA`),
/// the recitals (`GR.CONSID`), and the closing enacting formula
/// (`PREAMBLE.FINAL`).
#[derive(Serialize)]
pub struct Preamble {
    /// Opening formula (`<PREAMBLE.INIT>`), e.g. `"THE EUROPEAN PARLIAMENT AND THE COUNCIL …"`.
    pub init: String,
    /// Legal basis citations (`<VISA>` elements inside `<GR.VISA>`),
    /// each rendered as plain text.
    pub visas: Vec<String>,
    /// Numbered recitals (`<CONSID>` elements inside `<GR.CONSID>`).
    pub recitals: Vec<Recital>,
    /// Closing enacting formula (`<PREAMBLE.FINAL>`), e.g. `"HAVE ADOPTED THIS REGULATION:"`.
    pub enacting_formula: String,
}

/// A single numbered recital (`<CONSID>`).
///
/// In Formex the content is wrapped in a numbered paragraph (`<NP>`):
/// `<NO.P>` holds the label and `<TXT>` holds the body.
#[derive(Serialize)]
pub struct Recital {
    /// The recital label, e.g. `"(1)"`.
    pub number: String,
    /// The plain-text body of the recital.
    pub text: String,
}

/// The operative body of the act (`<ENACTING.TERMS>`).
#[derive(Serialize)]
pub struct EnactingTerms {
    /// Top-level chapters, mapped from `<DIVISION>` elements directly inside
    /// `<ENACTING.TERMS>`.
    pub chapters: Vec<Chapter>,
}

/// A chapter of the act (`<DIVISION>` at the top level of `<ENACTING.TERMS>`).
///
/// Chapters either contain sections (themselves containing articles) or
/// articles directly — never both.
#[derive(Serialize)]
pub struct Chapter {
    /// Chapter heading, e.g. `"CHAPTER I"` (from `<TITLE><TI>`).
    pub title: String,
    /// Optional chapter subtitle, e.g. `"General provisions"` (from `<TITLE><STI>`).
    pub subtitle: Option<String>,
    /// Either sections (each grouping articles) or articles directly —
    /// the two forms never mix within a single chapter.
    pub contents: ChapterContents,
}

/// Discriminates whether a chapter is sub-divided into sections.
#[derive(Serialize)]
pub enum ChapterContents {
    /// The chapter groups its articles under named sections.
    Sections(Vec<Section>),
    /// The chapter contains articles directly, with no section level.
    Articles(Vec<Article>),
}

/// A section within a chapter (`<DIVISION>` nested inside a top-level `<DIVISION>`).
#[derive(Serialize)]
pub struct Section {
    /// Section heading, e.g. `"SECTION 1"` (from `<TITLE><TI>`).
    pub title: String,
    /// Optional section subtitle (from `<TITLE><STI>`); present only in some acts.
    pub subtitle: Option<String>,
    /// Articles in this section. Sections are never nested further.
    pub articles: Vec<Article>,
}

/// A single article (`<ARTICLE>`).
#[derive(Serialize)]
pub struct Article {
    /// Article number as printed, e.g. `"Article 6"` (from `<TI.ART>`).
    pub number: String,
    /// Optional article title, e.g. `"Classification rules for high-risk AI systems"`
    /// (from `<STI.ART>`).
    pub title: Option<String>,
    /// The paragraphs of the article. Single-paragraph articles still use
    /// this vec (length 1, `number: None`).
    pub paragraphs: Vec<Paragraph>,
}

/// A numbered paragraph within an article (`<PARAG>`).
///
/// Each paragraph consists of an optional number label followed by one or
/// more alineas (text blocks). When an article has no `<PARAG>` wrappers its
/// `<ALINEA>` children are grouped into a single paragraph with `number: None`.
#[derive(Debug, PartialEq, Serialize)]
pub struct Paragraph {
    /// Paragraph number, e.g. `"1."` (from `<NO.PARAG>`). `None` for articles
    /// that use bare `<ALINEA>` elements without a `<PARAG>` wrapper.
    pub number: Option<String>,
    /// Subparagraphs of this paragraph. A plain alinea becomes a
    /// [`Subparagraph::Text`]; an alinea that contains a `<LIST>` (with its
    /// optional intro `<P>`) becomes a [`Subparagraph::List`]; a `<GR.TBL>`
    /// or bare `<TBL>` element becomes a [`Subparagraph::Table`].
    pub alineas: Vec<Subparagraph>,
}

/// A content element within a [`Paragraph`].
///
/// Covers plain text and full list groups (intro + items). The recursive
/// structure — `List` items are themselves `Vec<Subparagraph>` — handles
/// nesting without separate fields.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Subparagraph {
    /// Plain text, or a single numbered list item.
    ///
    /// `number` is `Some("(a)")` when this is a numbered entry in a list,
    /// and absent for plain text blocks.
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        number: Option<String>,
    },
    /// A list group: optional item label (present when this list is itself a
    /// numbered entry in a parent list), intro text, and the list items.
    List(ListBlock),
    /// A table parsed from a `<GR.TBL>` element.
    Table(Table),
}

/// A single cell within a [`Row`] (`<CELL>`).
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    /// Plain-text content of the cell. Empty string for `<IE/>` (idem/empty marker).
    pub text: String,
    /// `true` when the cell carries `TYPE="HEADER"`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_header: bool,
}

/// A row within a [`Table`] (`<ROW>`).
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// The cells in this row.
    pub cells: Vec<Cell>,
    /// Number of cells (convenience field matching `cells.len()`).
    pub cell_count: usize,
    /// `true` when the row carries `TYPE="HEADER"`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_header: bool,
}

/// A table parsed from a `<TBL>` element inside `<GR.TBL>`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Number of columns declared in the `COLS` attribute of `<TBL>`.
    pub col_count: usize,
    /// Optional table title (from `<TITLE><TI>`). Omitted from JSON when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The rows of the table body (`<CORPUS><ROW>`).
    pub rows: Vec<Row>,
    /// Number of rows (convenience field matching `rows.len()`).
    pub row_count: usize,
}

/// A list: optional item label, intro text, and items.
///
/// `number` is `Some("(c)")` when this list is itself a numbered item inside a
/// parent list; `None` for top-level lists.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ListBlock {
    /// Item label in a parent list, e.g. `"(c)"`. Omitted from JSON when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    /// The text that introduces the list (may be empty).
    pub intro: String,
    /// The items of the list, each itself a [`Subparagraph`].
    pub items: Vec<Subparagraph>,
}

/// A titled content section within an [`Annex`] (`<GR.SEQ>`).
///
/// Used when an annex organises its content under named headings.  For annexes
/// that consist of flat numbered paragraphs or plain text, [`AnnexContent::Paragraphs`]
/// is used instead.
#[derive(Serialize)]
pub struct AnnexSection {
    /// Section heading (from `<TITLE><TI>`).
    pub title: String,
    /// Content items nested inside this section.
    pub alineas: Vec<Subparagraph>,
}

/// Discriminates the top-level structure of an annex.
#[derive(Serialize)]
pub enum AnnexContent {
    /// The annex is divided into titled sections (`<GR.SEQ>`).
    Sections(Vec<AnnexSection>),
    /// The annex contains flat content: numbered paragraphs, lists, or plain text.
    Paragraphs(Vec<Paragraph>),
}

/// A parsed annex file (`<ANNEX>`).
#[derive(Serialize)]
pub struct Annex {
    /// Annex identifier, e.g. `"ANNEX I"` (from `<TITLE><TI>`).
    pub number: String,
    /// Optional descriptive subtitle (from `<TITLE><STI>`); present only in some annexes.
    pub subtitle: Option<String>,
    /// Top-level content: either titled sections or flat paragraphs.
    pub content: AnnexContent,
}
