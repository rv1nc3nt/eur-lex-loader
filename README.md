# eur-lex-loader

A Rust library and command-line tool for parsing EU acts (regulations and
directives) published in [Formex 4](https://op.europa.eu/en/web/eu-vocabularies/formex)
XML format and converting them to JSON.

The library extracts the full document structure: title, preamble (legal bases
and recitals), enacting terms (chapters, sections, and articles with nested
lists), annexes, and a flat definitions map when the act contains a Definitions
article.

---

## Getting the data

EU acts are published as Formex XML files in the
[Cellar](https://op.europa.eu/en/web/cellar) repository maintained by the
Publications Office of the European Union. No API key or account is required.

### Finding the CELEX number

Every EU legal act has a CELEX number. The format for regulations is:

```
3 YYYY R NNNN
│  │   │  └─ sequential number within the year
│  │   └─ document type (R = Regulation, L = Directive)
│  └─ year of publication
└─ sector (3 = secondary legislation)
```

Examples:
| Act | CELEX |
|---|---|
| EU AI Act (2024) | `32024R1689` |
| GDPR (2016) | `32016R0679` |
| DSA (2022) | `32022R2065` |
| EU Trade Mark Regulation (2017) | `32017R1001` |
| Copyright in the Digital Single Market Directive (2019) | `32019L0790` |

The CELEX number appears in the EUR-Lex URL for any act, e.g.:
`https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689`

### Downloading Formex XML

Use the Cellar REST API to download a ZIP archive of all Formex files for an
act. Pass the CELEX number directly in the URL:

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

The compiled binary is at `target/release/eur_lex_loader`.

---

## Usage

```
eur_lex_loader [OPTIONS] [DIR]

Arguments:
  [DIR]  Path to a local Formex act directory

Options:
  -c, --celex <CELEX>  Fetch from EUR-Lex Cellar by CELEX number (e.g. 32022R2065)
  -o, --output <FILE>  Write JSON output to FILE instead of stdout
      --compact        Output compact JSON (default: pretty-printed)
  -h, --help           Print help
  -V, --version        Print version
```

`DIR` and `--celex` are mutually exclusive. Running with no arguments prints help.

### Examples

```bash
# Fetch the DSA directly from EUR-Lex and pretty-print to stdout
eur_lex_loader -c 32022R2065

# Fetch the EU AI Act and write compact JSON to a file
eur_lex_loader -c 32024R1689 --compact --output ai_act.json

# Parse a previously downloaded regulation
eur_lex_loader data/MY_REGULATION

# Write compact JSON to a file
eur_lex_loader data/MY_REGULATION --compact --output regulation.json

# Pipe pretty-printed JSON into jq
eur_lex_loader data/MY_REGULATION | jq '.preamble.recitals | length'
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
                    // A plain paragraph:
                    { "Text": { "text": "The purpose of this Regulation …" } },
                    // A <P> intro + <LIST> collapsed into a single List block:
                    { "List": {
                        "intro": "The following practices shall be prohibited:",
                        "items": [
                          { "Text": { "text": "…", "number": "(a)" } },
                          // An item that itself has a nested list:
                          { "List": { "number": "(b)", "intro": "…",
                                      "items": [
                                        { "Text": { "text": "…", "number": "(i)" } }
                                      ] } }
                        ]
                    } }
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
      // Annexes with titled GR.SEQ sub-divisions use Sections:
      "content": {
        "Sections": [
          {
            "title": "Part A",
            "alineas": [
              { "Text": { "text": "…" } },
              { "List": { "intro": "…", "items": [
                  { "Text": { "text": "…", "number": "(a)" } }
              ] } }
            ]
          }
        ]
      }
      // Annexes with flat numbered items or plain text use Paragraphs:
      // "content": {
      //   "Paragraphs": [
      //     { "number": "1.", "alineas": [ { "Text": { "text": "…" } } ] },
      //     { "number": "2.", "alineas": [ { "List": { "intro": "…", "items": [ "…" ] } } ] }
      //   ]
      // }
    }
  ],

  // Present only when the act contains a Definitions article.
  // Key: defined term. Value: full definition text as it appears in the act,
  // including the term in curly quotes (e.g. "AI system" means …).
  "definitions": {
    "AI system": "\u201CAI system\u201D means a machine-based system …",
    "high-risk AI system": "\u201Chigh-risk AI system\u201D means …",
    "…": "…"
  }
}
```

`number` is omitted from the JSON when absent (plain text blocks and top-level
lists). `definitions` is omitted when the act has no Definitions article.

---

## Library usage

Add to `Cargo.toml`:

```toml
[dependencies]
eur-lex-loader = { path = "…" }
```

```rust
use eur_lex_loader::loader::load_act;
use std::path::Path;

fn main() -> Result<(), eur_lex_loader::error::Error> {
    let act = load_act(Path::new("data/MY_REGULATION"))?;
    println!("Title: {}", act.title);
    println!("Recitals: {}", act.preamble.recitals.len());
    if let Some(def) = act.definitions.get("AI system") {
        println!("AI system: {def}");
    }
    Ok(())
}
```

The public API is documented with `cargo doc --open`.

---

## Running the tests

```bash
cargo test
```

Unit tests live alongside their source modules. Integration tests validate the
full parse of four different EU legislative acts against known structural counts:

| File | Act | Articles | Recitals | Definitions |
|---|---|---|---|---|
| `tests/eu_ai_act.rs` | EU AI Act (32024R1689) | 113 | 180 | 68 |
| `tests/dsa.rs` | Digital Services Act (32022R2065) | 93 | 156 | 27 |
| `tests/dsma.rs` | Copyright in the Digital Single Market (32019L0790) | 32 | 86 | 6 |
| `tests/trademark_act.rs` | EU Trade Mark Regulation (32017R1001) | 212 | 48 | — |

---

## Limitations

- Only the English Formex 4 format is tested.
- Footnote bodies (`<NOTE>`) are dropped during text extraction; only the
  surrounding sentence is preserved.
- Formex elements not covered by the model (e.g. tables, images) are silently
  ignored; plain text inside them is still extracted where possible.
