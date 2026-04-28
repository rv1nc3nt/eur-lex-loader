use roxmltree::Node;

pub fn extract_text(node: Node) -> String {
    let raw = collect_text(node);
    normalize_whitespace(&raw.replace('\u{00A0}', " "))
}

fn collect_text(node: Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() {
            match child.tag_name().name() {
                "NOTE" => {}
                "QUOT.START" => {
                    out.push('\u{201C}');
                    // text following the element is a sibling text node, handled by loop
                }
                "QUOT.END" => {
                    out.push('\u{201D}');
                }
                _ => out.push_str(&collect_text(child)),
            }
        }
    }
    out
}

fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_space = true;
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
    if result.ends_with(' ') {
        result.pop();
    }
    result
}
