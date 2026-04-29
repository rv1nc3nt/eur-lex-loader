use roxmltree::Node;

/// Extracts plain text from a Formex XML element, handling mixed content correctly.
///
/// Formex documents use *mixed content*: inline tags such as `<HT>`, `<DATE>`,
/// `<NOTE>`, and `<QUOT.START>` appear mid-sentence alongside raw text nodes.
/// This function walks the full subtree and applies the following rules:
///
/// - Text nodes are included verbatim.
/// - `<NOTE>` elements are dropped entirely (footnote bodies are irrelevant for
///   plain-text extraction; the surrounding sentence punctuation is preserved
///   because it lives in sibling text nodes, not inside the note).
/// - `<QUOT.START>` emits a Unicode opening double quotation mark (`\u{201C}`).
/// - `<QUOT.END>` emits a Unicode closing double quotation mark (`\u{201D}`).
/// - All other elements are recursed into transparently.
/// - Non-breaking spaces (`\u{00A0}`) are converted to regular spaces.
/// - Runs of whitespace (including newlines from pretty-printed XML) are
///   collapsed to a single space and leading/trailing whitespace is trimmed.
///
/// # Example
///
/// ```
/// use eur_lex_loader::text::extract_text;
///
/// let xml = r#"<P>Hello <NOTE NOTE.ID="n1"><P>fn</P></NOTE> world.</P>"#;
/// let doc = roxmltree::Document::parse(xml).unwrap();
/// assert_eq!(extract_text(doc.root_element()), "Hello world.");
/// ```
pub fn extract_text(node: Node) -> String {
    let raw = collect_text(node);
    normalize_whitespace(&raw.replace('\u{00A0}', " "))
}

/// Recursively collects raw text from a node's subtree without post-processing.
fn collect_text(node: Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            match child.tag_name().name() {
                // Footnote bodies are noise for plain-text purposes; drop them.
                // The comma/period after the footnote marker is a sibling text
                // node and is therefore preserved by the loop.
                "NOTE" => {}
                // QUOT.START / QUOT.END carry no text content themselves;
                // they are markers for the opening and closing of a quoted span.
                "QUOT.START" => out.push('\u{201C}'),
                "QUOT.END" => out.push('\u{201D}'),
                _ => out.push_str(&collect_text(child)),
            }
        }
    }
    out
}

/// Collapses runs of whitespace to a single space and trims the result.
pub(crate) fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_space = true; // start true so leading whitespace is trimmed
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    // Trim a trailing space that was pushed before the string ended.
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_extract(xml: &str) -> String {
        let doc = roxmltree::Document::parse(xml).unwrap();
        extract_text(doc.root_element())
    }

    #[test]
    fn plain_text() {
        assert_eq!(parse_and_extract("<P>Hello world</P>"), "Hello world");
    }

    #[test]
    fn nested_elements_transparent() {
        // HT (highlighting) elements are transparent wrappers.
        assert_eq!(
            parse_and_extract("<P>foo <HT TYPE=\"UC\">bar</HT> baz</P>"),
            "foo bar baz"
        );
    }

    #[test]
    fn note_suppressed() {
        // The NOTE body is dropped; the comma after the marker is kept.
        let xml = r#"<P>See Article 5<NOTE NOTE.ID="E0001"><P>body</P></NOTE>, paragraph 1.</P>"#;
        assert_eq!(parse_and_extract(xml), "See Article 5, paragraph 1.");
    }

    #[test]
    fn quot_markers_converted() {
        let xml = r#"<P><QUOT.START CODE="2018"/>hello<QUOT.END CODE="2019"/></P>"#;
        assert_eq!(parse_and_extract(xml), "\u{201C}hello\u{201D}");
    }

    #[test]
    fn nbsp_converted_to_space() {
        // Non-breaking spaces appear in identifiers like "Article\u{a0}1".
        let xml = "<P>Article\u{a0}1</P>";
        assert_eq!(parse_and_extract(xml), "Article 1");
    }

    #[test]
    fn whitespace_collapsed() {
        assert_eq!(normalize_whitespace("  foo   bar  "), "foo bar");
    }

    #[test]
    fn empty_string() {
        assert_eq!(normalize_whitespace(""), "");
    }

    #[test]
    fn only_whitespace() {
        assert_eq!(normalize_whitespace("   "), "");
    }
}
