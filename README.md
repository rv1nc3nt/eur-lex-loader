# euro-lex-loader

A Rust library and command-line tool for parsing EU regulations published in
[Formex 4](https://op.europa.eu/en/web/eu-vocabularies/formex) XML format and
converting them to JSON.

The library extracts the full document structure: title, preamble (legal bases
and recitals), enacting terms (chapters, sections, and articles with nested
lists), and annexes.

---

## Getting the data

EU regulations are published as Formex XML files in the
[Cellar](https://op.europa.eu/en/web/cellar) repository maintained by the
Publications Office of the European Union. No API key or account is required.

### Finding the CELEX number

Every EU legal act has a CELEX number. The format for regulations is:

```
3 YYYY R NNNN
│  │   │  └─ sequential number within the year
│  │   └─ document type (R = Regulation)
│  └─ year of publication
└─ sector (3 = secondary legislation)
```

Examples:
| Regulation | CELEX |
|---|---|
| EU AI Act (2024) | `32024R1689` |
| GDPR (2016) | `32016R0679` |
| DSA (2022) | `32022R2065` |

The CELEX number appears in the EUR-Lex URL for any regulation, e.g.:
`https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689`

### Downloading Formex XML

Use the Cellar REST API to download a ZIP archive of all Formex files for a
regulation. Pass the CELEX number directly in the URL:

```bash
curl -L \
  -H "Accept: application/zip;mtype=fmx4" \
  -H "Accept-Language: eng" \
  "http://publications.europa.eu/resource/celex/32024R1689" \
  -o regulation.zip
```

- `-L` follows redirects (required — the API returns a redirect)
- `mtype=fmx4` requests the Formex 4 package
- `Accept-Language: eng` requests the English language version; use `fra`,
  `deu`, `ita`, etc. for other languages

Unzip the archive into a directory:

```bash
unzip regulation.zip -d data/MY_REGULATION
```

The directory will contain several `.fmx.xml` files:

| Filename pattern | Content |
|---|---|
| `*.000101.fmx.xml` | Main act (title, preamble, enacting terms) |
| `*.012401.fmx.xml` and above | Annexes, one file each |
| `*.doc.fmx.xml` | Registry listing all files in order |
| `*.toc.fmx.xml` | Table of contents (not used by this tool) |

> **Rate limiting**: keep concurrent requests below 5 per IP address.

---

## Building

```bash
cargo build --release
```

The compiled binary is at `target/release/euro-lex-loader`.

---

## Usage

```
euro-lex-loader [OPTIONS] [DIR]

Arguments:
  [DIR]  Path to the Formex regulation directory [default: data/EU_AI_ACT]

Options:
  -o, --output <FILE>  Write JSON output to FILE instead of stdout
  -c, --compact        Output compact JSON (default: pretty-printed)
  -h, --help           Print help
  -V, --version        Print version
```

### Examples

```bash
# Parse the bundled EU AI Act example and pretty-print to stdout
euro-lex-loader

# Parse a downloaded regulation
euro-lex-loader data/MY_REGULATION

# Write compact JSON to a file
euro-lex-loader data/MY_REGULATION --compact --output regulation.json

# Pipe pretty-printed JSON into jq
euro-lex-loader data/MY_REGULATION | jq '.preamble.recitals | length'
```

---

## Output format

The tool outputs a single JSON object with the following shape:

```jsonc
{
  "title": "Regulation (EU) 2024/1689 …",

  "preamble": {
    "init": "THE EUROPEAN PARLIAMENT AND THE COUNCIL …",
    "visas": ["Having regard to …", "…"],
    "recitals": [
      { "number": "(1)", "text": "The purpose of this Regulation …" },
      "…"
    ],
    "enacting_formula": "HAVE ADOPTED THIS REGULATION:"
  },

  "enacting_terms": {
    "chapters": [
      {
        "title": "CHAPTER I",
        "subtitle": "General provisions",
        // A chapter contains either sections or articles directly:
        "contents": {
          "Articles": [
            {
              "number": "Article 1",
              "title": "Subject matter",
              "paragraphs": [
                {
                  "number": "1.",
                  "alineas": [
                    { "Paragraph": "The purpose of this Regulation …" },
                    { "ListItem": { "number": "(a)", "text": "…" } },
                    // Items with nested lists carry sub_items:
                    { "ListItem": { "number": "(b)", "text": "…",
                                    "sub_items": [
                                      { "ListItem": { "number": "(i)", "text": "…" } }
                                    ] } }
                  ]
                }
              ]
            }
          ]
        }
      }
    ]
  },

  "annexes": [
    {
      "number": "ANNEX I",
      "subtitle": "List of harmonised standards …",
      "content_blocks": [
        { "Paragraph": "…" },
        { "ListItem": { "number": "1.", "text": "…" } }
      ]
    }
  ]
}
```

`sub_items` is omitted from the JSON when empty.

---

## Library usage

Add to `Cargo.toml`:

```toml
[dependencies]
euro-lex-loader = { path = "…" }
```

```rust
use euro_lex_loader::loader::load_regulation;
use std::path::Path;

fn main() -> Result<(), euro_lex_loader::error::Error> {
    let reg = load_regulation(Path::new("data/MY_REGULATION"))?;
    println!("Title: {}", reg.title);
    println!("Recitals: {}", reg.preamble.recitals.len());
    Ok(())
}
```

The public API is documented with `cargo doc --open`.

---

## Running the tests

```bash
cargo test
```

Unit tests live alongside their source modules. The integration test in
`tests/eu_ai_act.rs` validates the full parse of the bundled EU AI Act against
known structural counts (113 articles, 180 recitals, 13 chapters, 13 annexes).

---

## Limitations

- Only the English Formex 4 format is tested.
- Footnote bodies (`<NOTE>`) are dropped during text extraction; only the
  surrounding sentence is preserved.
- Formex elements not covered by the model (e.g. tables, images) are silently
  ignored; plain text inside them is still extracted where possible.
