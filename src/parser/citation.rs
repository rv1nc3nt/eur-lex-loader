use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use roxmltree::Node;

use crate::model::{CitedActType, Citation, OjRef};

// Pattern A — prefix-regime style: Regulation (EU) 2022/2065, Directive (EC) No 207/2009,
// Regulation (EC) No 40/94 (old 2-digit-year form).
static RE_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(Regulation|Directive|Decision)\s+\((EU|EC|EEC|EURATOM)\)\s*(?:No\s+)?(\d+/\d+)",
    )
    .unwrap()
});

// Pattern B — suffix-regime style: Directive 2008/95/EC, Directive 89/104/EEC,
// Decision No 1247/2002/EC
static RE_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(Regulation|Directive|Decision)\s+(?:No\s+)?(\d{4}/\d+|\d+/\d+)/(EU|EC|EEC|EURATOM)\b",
    )
    .unwrap()
});

/// Converts a matched regex capture string to the corresponding [`CitedActType`].
fn to_act_type(s: &str) -> CitedActType {
    match s {
        "Regulation" => CitedActType::Regulation,
        "Directive" => CitedActType::Directive,
        _ => CitedActType::Decision,
    }
}

/// Collects text from all descendants, skipping `<NOTE>` subtrees entirely.
fn text_without_notes(node: Node) -> String {
    let mut buf = String::new();
    collect_text_skip_notes(node, &mut buf);
    buf.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Recursively collects text from `node`'s subtree, skipping `<NOTE>` elements entirely.
fn collect_text_skip_notes(node: Node, buf: &mut String) {
    if node.is_element() && node.tag_name().name() == "NOTE" {
        return;
    }
    if let Some(t) = node.text() {
        buf.push_str(t);
    }
    for child in node.children() {
        collect_text_skip_notes(child, buf);
    }
}

/// Collects all text from `node`'s subtree, including inside `<NOTE>` elements.
fn raw_text(node: Node) -> String {
    let mut buf = String::new();
    for desc in node.descendants() {
        if let Some(t) = desc.text() {
            buf.push_str(t);
        }
    }
    buf.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Returns the first `<REF.DOC.OJ>` element anywhere in `node`'s subtree.
fn find_oj_ref(node: Node) -> Option<OjRef> {
    for desc in node.descendants() {
        if desc.is_element() && desc.tag_name().name() == "REF.DOC.OJ" {
            let collection = desc.attribute("COLL")?.to_string();
            let number = desc.attribute("NO.OJ")?.to_string();
            let date = desc.attribute("DATE.PUB")?.to_string();
            let page = desc.attribute("PAGE.FIRST")?.parse().ok()?;
            return Some(OjRef { collection, number, date, page });
        }
    }
    None
}

/// Internal result of a single citation regex match within a text string.
struct CitMatch {
    /// Byte offset of the match start, used to pick the earlier match when both patterns fire.
    pos: usize,
    /// Regulation, Directive, or Decision.
    act_type: CitedActType,
    /// Jurisdictional regime code (e.g. `"EU"`, `"EC"`, `"EEC"`).
    regime: String,
    /// Year/number identifier (e.g. `"2022/2065"`, `"207/2009"`).
    number: String,
}

/// Returns the first citation match in `text`, considering both patterns.
/// For each NOTE, only the first match is used (it is the primary citation
/// the note is a footnote for; later mentions like "repealing X" are ignored).
fn first_match(text: &str) -> Option<CitMatch> {
    let prefix = RE_PREFIX.find(text).and_then(|m| {
        RE_PREFIX.captures_at(text, m.start()).map(|c| CitMatch {
            pos: m.start(),
            act_type: to_act_type(&c[1]),
            regime: c[2].to_string(),
            number: c[3].to_string(),
        })
    });
    let suffix = RE_SUFFIX.find(text).and_then(|m| {
        RE_SUFFIX.captures_at(text, m.start()).map(|c| CitMatch {
            pos: m.start(),
            act_type: to_act_type(&c[1]),
            regime: c[3].to_string(),
            number: c[2].to_string(),
        })
    });
    match (prefix, suffix) {
        (Some(p), Some(s)) => Some(if p.pos <= s.pos { p } else { s }),
        (Some(p), None) => Some(p),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    }
}

/// Returns all citation matches in `text` from both patterns.
fn all_matches(text: &str) -> Vec<(CitedActType, String, String)> {
    let mut out = Vec::new();
    for c in RE_PREFIX.captures_iter(text) {
        out.push((to_act_type(&c[1]), c[2].to_string(), c[3].to_string()));
    }
    for c in RE_SUFFIX.captures_iter(text) {
        out.push((to_act_type(&c[1]), c[3].to_string(), c[2].to_string()));
    }
    out
}

/// Collects all direct `<NOTE>` elements in `node`'s subtree (non-recursive
/// into NOTEs — nested notes are not collected separately).
fn collect_notes<'a>(node: Node<'a, 'a>) -> Vec<Node<'a, 'a>> {
    let mut notes = Vec::new();
    collect_notes_rec(node, &mut notes);
    notes
}

/// Recursive implementation of [`collect_notes`], descending into non-`NOTE` children.
fn collect_notes_rec<'a>(node: Node<'a, 'a>, notes: &mut Vec<Node<'a, 'a>>) {
    for child in node.children() {
        if child.is_element() {
            if child.tag_name().name() == "NOTE" {
                notes.push(child);
            } else {
                collect_notes_rec(child, notes);
            }
        }
    }
}

fn dedup_key(act_type: &CitedActType, number: &str) -> (CitedActType, String) {
    (act_type.clone(), number.to_string())
}

/// Extracts all citations from the subtree rooted at `node`.
///
/// Algorithm:
/// 1. Scan each `<NOTE>` descendant; take only the **first** citation match per
///    note (the primary act the note footnotes); assign its `<REF.DOC.OJ>` if present.
/// 2. Scan the text of `node` excluding `<NOTE>` subtrees for inline citations not
///    already found in step 1.
/// 3. Deduplicate by `(act_type, number)` — note entries win over inline entries.
pub(crate) fn extract_citations(node: Node) -> Vec<Citation> {
    let mut result: Vec<Citation> = Vec::new();
    let mut seen: HashSet<(CitedActType, String)> = HashSet::new();

    // Step 1 — NOTE-anchored citations
    for note in collect_notes(node) {
        let text = raw_text(note);
        if let Some(m) = first_match(&text) {
            let key = dedup_key(&m.act_type, &m.number);
            if seen.insert(key) {
                result.push(Citation {
                    act_type: m.act_type,
                    regime: Some(m.regime),
                    number: m.number,
                    oj_ref: find_oj_ref(note),
                });
            }
        }
    }

    // Step 2 — inline citations (text with NOTE subtrees excluded)
    let inline = text_without_notes(node);
    for (act_type, regime, number) in all_matches(&inline) {
        let key = dedup_key(&act_type, &number);
        if seen.insert(key) {
            result.push(Citation { act_type, regime: Some(regime), number, oj_ref: None });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses a raw XML string into a `roxmltree::Document`, panicking on error.
    fn parse(xml: &str) -> roxmltree::Document<'_> {
        roxmltree::Document::parse(xml).unwrap()
    }

    /// Parses `xml` and extracts citations from the root element.
    fn citations(xml: &str) -> Vec<Citation> {
        let doc = parse(xml);
        extract_citations(doc.root_element())
    }

    /// Constructs an [`OjRef`] from its four fields for use in assertions.
    fn oj(collection: &str, number: &str, date: &str, page: u32) -> OjRef {
        OjRef { collection: collection.into(), number: number.into(), date: date.into(), page }
    }

    /// Constructs a [`Citation`] with `regime` set, for use in assertions.
    fn cit(act_type: CitedActType, regime: &str, number: &str, oj_ref: Option<OjRef>) -> Citation {
        Citation { act_type, regime: Some(regime.into()), number: number.into(), oj_ref }
    }

    // ── 1. NOTE with (EC) No style and REF.DOC.OJ ────────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (1)

    #[test]
    /// NOTE containing `(EC) No 207/2009` prefix-style citation with a `REF.DOC.OJ` element.
    fn note_ec_no_style_with_oj_ref() {
        let xml = r#"<CONSID><NP><NO.P>(1)</NO.P><TXT>Council Regulation (EC) No 207/2009<NOTE NOTE.ID="E0002" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Council Regulation (EC) No 207/2009 of <DATE ISO="20090226">26 February 2009</DATE> on the European Union trade mark (<REF.DOC.OJ COLL="L" DATE.PUB="20090324" NO.OJ="078" PAGE.FIRST="1">OJ L 78, 24.3.2009, p. 1</REF.DOC.OJ>).</P></NOTE> has been substantially amended.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            cit(CitedActType::Regulation, "EC", "207/2009", Some(oj("L", "078", "20090324", 1)))
        );
    }

    // ── 2. NOTE with modern (EU) year/number style and REF.DOC.OJ ────────────
    // Source: data/32024R1689/L_202401689EN.000101.fmx.xml, recital (11)

    #[test]
    /// NOTE containing modern `(EU) 2022/2065` year/number style citation with a `REF.DOC.OJ` element.
    fn note_eu_modern_style_with_oj_ref() {
        let xml = r#"<CONSID><NP><NO.P>(11)</NO.P><TXT>Regulation (EU) 2022/2065 of the European Parliament and of the Council<NOTE NOTE.ID="E0015" NUMBERING.CONTINUED="YES"><P>Regulation (EU) 2022/2065 of the European Parliament and of the Council of <DATE ISO="20221019">19 October 2022</DATE> on a Single Market For Digital Services and amending Directive 2000/31/EC (Digital Services Act) (<REF.DOC.OJ COLL="L" DATE.PUB="20221027" NO.OJ="277" PAGE.FIRST="1">OJ L 277, 27.10.2022, p. 1</REF.DOC.OJ>).</P></NOTE>.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            cit(CitedActType::Regulation, "EU", "2022/2065", Some(oj("L", "277", "20221027", 1)))
        );
    }

    // ── 3. NOTE with (EU) No style and REF.DOC.OJ ────────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (16) — NOTE only

    #[test]
    /// NOTE with `(EU) No 608/2013` style; only the first citation is extracted, ignoring the "repealing" mention.
    fn note_eu_no_style_with_oj_ref() {
        let xml = r#"<CONSID><NP><NO.P>(16)</NO.P><TXT>x<NOTE NOTE.ID="E0008" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Regulation (EU) No 608/2013 of the European Parliament and of the Council of <DATE ISO="20130612">12 June 2013</DATE> concerning customs enforcement of intellectual property rights and repealing Council Regulation (EC) No 1383/2003 (<REF.DOC.OJ COLL="L" DATE.PUB="20130629" NO.OJ="181" PAGE.FIRST="15">OJ L 181, 29.6.2013, p. 15</REF.DOC.OJ>).</P></NOTE>.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        // Only the first citation in the note is extracted; repealing mention is ignored.
        let eu_608 = result.iter().find(|c| c.number == "608/2013");
        assert!(eu_608.is_some(), "expected 608/2013 citation");
        let c = eu_608.unwrap();
        assert_eq!(c.regime, Some("EU".into()));
        assert_eq!(c.oj_ref, Some(oj("L", "181", "20130629", 15)));
        let ec_1383 = result.iter().find(|c| c.number == "1383/2003");
        assert!(ec_1383.is_none(), "secondary 'repealing' mention must not be extracted");
    }

    // ── 4. Inline citation (EU) No style without NOTE ─────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (18)

    #[test]
    /// Inline `(EU) No 608/2013` prefix-style citation with no NOTE and therefore no OJ ref.
    fn inline_citation_eu_no_style_no_oj_ref() {
        let xml = r#"<CONSID><NP><NO.P>(18)</NO.P><TXT>Article 28 of Regulation (EU) No 608/2013 provides that a right holder is to be liable for damages towards the holder of the goods where, inter alia, the goods in question are subsequently found not to infringe an intellectual property right.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], cit(CitedActType::Regulation, "EU", "608/2013", None));
    }

    // ── 5. Multiple inline citations with no NOTEs ────────────────────────────
    // Source: data/32024R1689/L_202401689EN.000101.fmx.xml, recital (14)

    #[test]
    /// Three distinct inline citations in one element, all without NOTEs, yields three entries with no OJ ref.
    fn inline_multiple_citations_no_notes() {
        let xml = r#"<CONSID><NP><NO.P>(14)</NO.P><TXT>The notion of biometric data used in this Regulation should be interpreted in light of the notion of biometric data as defined in Article 4, point (14) of Regulation (EU) 2016/679, Article 3, point (18) of Regulation (EU) 2018/1725 and Article 3, point (13) of Directive (EU) 2016/680.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&cit(CitedActType::Regulation, "EU", "2016/679", None)));
        assert!(result.contains(&cit(CitedActType::Regulation, "EU", "2018/1725", None)));
        assert!(result.contains(&cit(CitedActType::Directive, "EU", "2016/680", None)));
    }

    // ── 6. Deduplication: NOTE entry beats matching inline mention ────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (16)

    #[test]
    /// When the same act appears both inline and in a NOTE, the NOTE entry (with OJ ref) wins and only one entry remains.
    fn dedup_note_beats_inline() {
        // "(EU) No 608/2013" appears both in TXT (inline) and in the NOTE with OJ ref.
        let xml = r#"<CONSID><NP><NO.P>(16)</NO.P><TXT>In performing customs controls, the customs authorities should make use of the powers and procedures laid down in Regulation (EU) No 608/2013 of the European Parliament and the Council<NOTE NOTE.ID="E0008" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Regulation (EU) No 608/2013 of the European Parliament and of the Council of <DATE ISO="20130612">12 June 2013</DATE> concerning customs enforcement of intellectual property rights and repealing Council Regulation (EC) No 1383/2003 (<REF.DOC.OJ COLL="L" DATE.PUB="20130629" NO.OJ="181" PAGE.FIRST="15">OJ L 181, 29.6.2013, p. 15</REF.DOC.OJ>).</P></NOTE>, also at the request of the right holders.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        let eu_608: Vec<_> = result.iter().filter(|c| c.number == "608/2013").collect();
        assert_eq!(eu_608.len(), 1, "608/2013 must appear exactly once after dedup");
        assert!(eu_608[0].oj_ref.is_some(), "note entry (with OJ ref) must win over inline");
    }

    // ── 7. Old EEC suffix style ───────────────────────────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (2), third NOTE

    #[test]
    /// Suffix-regime `89/104/EEC` style NOTE is recognised and its OJ ref is captured.
    fn old_eec_suffix_style() {
        let xml = r#"<CONSID><NP><NO.P>(2)</NO.P><TXT>x<NOTE NOTE.ID="E0005" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>First Council Directive 89/104/EEC of <DATE ISO="19881221">21 December 1988</DATE> to approximate the laws of the Member States relating to trade marks (<REF.DOC.OJ COLL="L" DATE.PUB="19890211" NO.OJ="040" PAGE.FIRST="1">OJ L 40, 11.2.1989, p. 1</REF.DOC.OJ>).</P></NOTE>.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        let c = result.iter().find(|c| c.number == "89/104");
        assert!(c.is_some(), "expected Directive 89/104");
        let c = c.unwrap();
        assert_eq!(c.act_type, CitedActType::Directive);
        assert_eq!(c.regime, Some("EEC".into()));
        assert_eq!(c.oj_ref, Some(oj("L", "040", "19890211", 1)));
    }

    // ── 8. EC suffix style ───────────────────────────────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (2), fourth NOTE

    #[test]
    /// Suffix-regime `2008/95/EC` style NOTE is recognised and its OJ ref is captured.
    fn ec_suffix_style() {
        let xml = r#"<CONSID><NP><NO.P>(2)</NO.P><TXT>x<NOTE NOTE.ID="E0006" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Directive 2008/95/EC of the European Parliament and of the Council of <DATE ISO="20081022">22 October 2008</DATE> to approximate the laws of the Member States relating to trade marks (<REF.DOC.OJ COLL="L" DATE.PUB="20081108" NO.OJ="299" PAGE.FIRST="25">OJ L 299, 8.11.2008, p. 25</REF.DOC.OJ>).</P></NOTE>.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        let c = result.iter().find(|c| c.number == "2008/95");
        assert!(c.is_some(), "expected Directive 2008/95");
        let c = c.unwrap();
        assert_eq!(c.act_type, CitedActType::Directive);
        assert_eq!(c.regime, Some("EC".into()));
        assert_eq!(c.oj_ref, Some(oj("L", "299", "20081108", 25)));
    }

    // ── 9. Three NOTEs in one recital ─────────────────────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (2)

    #[test]
    /// Three NOTEs in one recital each contribute one citation; an additional inline mention is also captured.
    fn multiple_notes_in_one_recital() {
        let xml = r#"<CONSID><NP><NO.P>(2)</NO.P><TXT>Council Regulation (EC) No 40/94<NOTE NOTE.ID="E0004" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Council Regulation (EC) No 40/94 of <DATE ISO="19931220">20 December 1993</DATE> on the Community trade mark (<REF.DOC.OJ COLL="L" DATE.PUB="19940114" NO.OJ="011" PAGE.FIRST="1">OJ L 11, 14.1.1994, p. 1</REF.DOC.OJ>).</P></NOTE>, which was codified in 2009 as Regulation (EC) No 207/2009, created a system of trade mark protection specific to the Union which provided for the protection of trade marks at the level of the Union, in parallel to the protection of trade marks available at the level of the Member States in accordance with the national trade mark systems, harmonised by Council Directive 89/104/EEC<NOTE NOTE.ID="E0005" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>First Council Directive 89/104/EEC of <DATE ISO="19881221">21 December 1988</DATE> to approximate the laws of the Member States relating to trade marks (<REF.DOC.OJ COLL="L" DATE.PUB="19890211" NO.OJ="040" PAGE.FIRST="1">OJ L 40, 11.2.1989, p. 1</REF.DOC.OJ>).</P></NOTE>, which was codified as Directive 2008/95/EC of the European Parliament and of the Council<NOTE NOTE.ID="E0006" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Directive 2008/95/EC of the European Parliament and of the Council of <DATE ISO="20081022">22 October 2008</DATE> to approximate the laws of the Member States relating to trade marks (<REF.DOC.OJ COLL="L" DATE.PUB="20081108" NO.OJ="299" PAGE.FIRST="25">OJ L 299, 8.11.2008, p. 25</REF.DOC.OJ>).</P></NOTE>.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        // Three NOTEs each contribute one citation with OJ ref.
        assert!(result.contains(&cit(CitedActType::Regulation, "EC", "40/94",   Some(oj("L", "011", "19940114", 1)))));
        assert!(result.contains(&cit(CitedActType::Directive,  "EEC", "89/104", Some(oj("L", "040", "19890211", 1)))));
        assert!(result.contains(&cit(CitedActType::Directive,  "EC", "2008/95", Some(oj("L", "299", "20081108", 25)))));
        // Inline text also mentions "207/2009" (no NOTE for it in this recital).
        assert!(result.contains(&cit(CitedActType::Regulation, "EC", "207/2009", None)));
    }

    // ── 10. Four NOTEs in one recital ─────────────────────────────────────────
    // Source: data/32024R1689/L_202401689EN.000101.fmx.xml, recital (10)

    #[test]
    /// Four NOTEs in one recital each yield a citation with an OJ ref, including mixed prefix and suffix styles.
    fn four_notes_in_one_recital() {
        let xml = r#"<CONSID><NP><NO.P>(10)</NO.P><TXT>The fundamental right to the protection of personal data is safeguarded in particular by Regulations (EU) 2016/679<NOTE NOTE.ID="E0011" NUMBERING.CONTINUED="YES"><P>Regulation (EU) 2016/679 of the European Parliament and of the Council of <DATE ISO="20160427">27 April 2016</DATE> on the protection of natural persons with regard to the processing of personal data and on the free movement of such data, and repealing Directive 95/46/EC (General Data Protection Regulation) (<REF.DOC.OJ COLL="L" DATE.PUB="20160504" NO.OJ="119" PAGE.FIRST="1">OJ L 119, 4.5.2016, p. 1</REF.DOC.OJ>).</P></NOTE> and (EU) 2018/1725<NOTE NOTE.ID="E0012" NUMBERING.CONTINUED="YES"><P>Regulation (EU) 2018/1725 of the European Parliament and of the Council of <DATE ISO="20181023">23 October 2018</DATE> on the protection of natural persons with regard to the processing of personal data by the Union institutions, bodies, offices and agencies and on the free movement of such data, and repealing Regulation (EC) No 45/2001 and Decision No 1247/2002/EC (<REF.DOC.OJ COLL="L" DATE.PUB="20181121" NO.OJ="295" PAGE.FIRST="39">OJ L 295, 21.11.2018, p. 39</REF.DOC.OJ>).</P></NOTE> of the European Parliament and of the Council and Directive (EU) 2016/680 of the European Parliament and of the Council<NOTE NOTE.ID="E0013" NUMBERING.CONTINUED="YES"><P>Directive (EU) 2016/680 of the European Parliament and of the Council of <DATE ISO="20160427">27 April 2016</DATE> on the protection of natural persons with regard to the processing of personal data by competent authorities for the purposes of the prevention, investigation, detection or prosecution of criminal offences or the execution of criminal penalties, and on the free movement of such data, and repealing Council Framework Decision 2008/977/JHA (<REF.DOC.OJ COLL="L" DATE.PUB="20160504" NO.OJ="119" PAGE.FIRST="89">OJ L 119, 4.5.2016, p. 89</REF.DOC.OJ>).</P></NOTE>. Directive 2002/58/EC of the European Parliament and of the Council<NOTE NOTE.ID="E0014" NUMBERING.CONTINUED="YES"><P>Directive 2002/58/EC of the European Parliament and of the Council of <DATE ISO="20020712">12 July 2002</DATE> concerning the processing of personal data and the protection of privacy in the electronic communications sector (Directive on privacy and electronic communications) (<REF.DOC.OJ COLL="L" DATE.PUB="20020731" NO.OJ="201" PAGE.FIRST="37">OJ L 201, 31.7.2002, p. 37</REF.DOC.OJ>).</P></NOTE> additionally protects private life and the confidentiality of communication.</TXT></NP></CONSID>"#;
        let result = citations(xml);
        assert!(result.contains(&cit(CitedActType::Regulation, "EU", "2016/679",  Some(oj("L", "119", "20160504", 1)))));
        assert!(result.contains(&cit(CitedActType::Regulation, "EU", "2018/1725", Some(oj("L", "295", "20181121", 39)))));
        assert!(result.contains(&cit(CitedActType::Directive,  "EU", "2016/680",  Some(oj("L", "119", "20160504", 89)))));
        assert!(result.contains(&cit(CitedActType::Directive,  "EC", "2002/58",   Some(oj("L", "201", "20020731", 37)))));
    }

    // ── 11. NOTE without any act citation is ignored ──────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, recital (1), second NOTE

    #[test]
    /// A NOTE whose text does not contain a recognisable act citation produces no results.
    fn note_without_act_citation_is_ignored() {
        let xml = r#"<CONSID><NP><NO.P>(1)</NO.P><TXT>amended several times<NOTE NOTE.ID="E0003" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>See Annex II.</P></NOTE>.</TXT></NP></CONSID>"#;
        assert!(citations(xml).is_empty());
    }

    // ── 12. NOTE with procedural text only is ignored ─────────────────────────
    // Source: data/32017R1001/L_2017154EN.01000101.xml, third visa

    #[test]
    /// A NOTE containing only procedural text (dates, positions) with no act reference is ignored.
    fn note_with_procedural_text_is_ignored() {
        let xml = r#"<VISA>Acting in accordance with the ordinary legislative procedure<NOTE NOTE.ID="E0001" NUMBERING="ARAB" TYPE="FOOTNOTE"><P>Position of the European Parliament of <DATE ISO="20170427">27 April 2017</DATE> (not yet published in the Official Journal) and decision of the Council of <DATE ISO="20170522">22 May 2017</DATE>.</P></NOTE>,</VISA>"#;
        assert!(citations(xml).is_empty());
    }

    // ── 13. Element with no citations at all ──────────────────────────────────
    // Constructed test — no real data file required.

    #[test]
    /// An element with plain prose and no act references returns an empty citation list.
    fn element_with_no_citations() {
        let xml = r#"<CONSID><NP><NO.P>(5)</NO.P><TXT>This Regulation applies without prejudice to existing procedural rules.</TXT></NP></CONSID>"#;
        assert!(citations(xml).is_empty());
    }

    // ── 14. Article paragraph with NOTE and REF.DOC.OJ ───────────────────────
    // Source: data/32024R1689/L_202401689EN.000101.fmx.xml — amending article paragraph

    #[test]
    /// Citation in an article `<PARAG>` (not a recital) with a NOTE and OJ ref is extracted correctly.
    fn article_paragraph_with_note_and_oj_ref() {
        let xml = r#"<PARAG><NO.PARAG>1.</NO.PARAG><ALINEA><P>In Annex I to Directive (EU) 2020/1828 of the European Parliament and of the Council<NOTE NOTE.ID="E0066" NUMBERING.CONTINUED="YES"><P>Directive (EU) 2020/1828 of the European Parliament and of the Council of <DATE ISO="20201125">25 November 2020</DATE> on representative actions for the protection of the collective interests of consumers and repealing Directive 2009/22/EC (<REF.DOC.OJ COLL="L" DATE.PUB="20201204" NO.OJ="409" PAGE.FIRST="1">OJ L 409, 4.12.2020, p. 1</REF.DOC.OJ>).</P></NOTE>, the following point is added:</P></ALINEA></PARAG>"#;
        let result = citations(xml);
        let c = result.iter().find(|c| c.number == "2020/1828");
        assert!(c.is_some(), "expected Directive (EU) 2020/1828");
        let c = c.unwrap();
        assert_eq!(c.act_type, CitedActType::Directive);
        assert_eq!(c.regime, Some("EU".into()));
        assert_eq!(c.oj_ref, Some(oj("L", "409", "20201204", 1)));
    }

    // ── 15. AnnexSection (GR.SEQ) level extraction ───────────────────────────
    // Constructed test matching the GR.SEQ pattern used in annex parsing.

    #[test]
    /// Citations inside a `<GR.SEQ>` annex section element are extracted correctly.
    fn annex_section_gr_seq_level() {
        let xml = r#"<GR.SEQ><TITLE><TI><P>Part A</P></TI></TITLE><P>As listed in Regulation (EU) 2024/1689<NOTE NOTE.ID="X1" NUMBERING.CONTINUED="YES"><P>Regulation (EU) 2024/1689 (<REF.DOC.OJ COLL="L" DATE.PUB="20240712" NO.OJ="1689" PAGE.FIRST="1">OJ L 1689, 12.7.2024, p. 1</REF.DOC.OJ>).</P></NOTE>.</P></GR.SEQ>"#;
        let result = citations(xml);
        let c = result.iter().find(|c| c.number == "2024/1689");
        assert!(c.is_some(), "expected Regulation (EU) 2024/1689 from GR.SEQ");
        let c = c.unwrap();
        assert_eq!(c.act_type, CitedActType::Regulation);
        assert_eq!(c.regime, Some("EU".into()));
        assert!(c.oj_ref.is_some());
    }
}
