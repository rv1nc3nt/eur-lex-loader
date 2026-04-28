pub struct Regulation {
    pub title: String,
    pub preamble: Preamble,
    pub enacting_terms: EnactingTerms,
    pub annexes: Vec<Annex>,
}

pub struct Preamble {
    pub init: String,
    pub visas: Vec<String>,
    pub recitals: Vec<Recital>,
    pub enacting_formula: String,
}

pub struct Recital {
    pub number: String,
    pub text: String,
}

pub struct EnactingTerms {
    pub chapters: Vec<Chapter>,
}

pub struct Chapter {
    pub title: String,
    pub subtitle: Option<String>,
    pub contents: ChapterContents,
}

pub enum ChapterContents {
    Sections(Vec<Section>),
    Articles(Vec<Article>),
}

pub struct Section {
    pub title: String,
    pub subtitle: Option<String>,
    pub articles: Vec<Article>,
}

pub struct Article {
    pub number: String,
    pub title: Option<String>,
    pub paragraphs: Vec<Paragraph>,
}

pub struct Paragraph {
    pub number: Option<String>,
    pub alineas: Vec<String>,
}

pub struct Annex {
    pub number: String,
    pub subtitle: Option<String>,
    pub content_blocks: Vec<ContentBlock>,
}

pub enum ContentBlock {
    Paragraph(String),
    ListItem { number: String, text: String },
    Section { title: String, blocks: Vec<ContentBlock> },
}
