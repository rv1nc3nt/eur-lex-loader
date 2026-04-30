# eur-lex-loader

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
| EU Trade Mark Regulation (2017) | `32017R1001` |

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

The compiled binary is at `target/release/eur_lex_loader`.

---

## Usage

```
eur_lex_loader [OPTIONS] [DIR]

Arguments:
  [DIR]  Path to a local Formex regulation directory

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
eur-lex-loader = { path = "…" }
```

```rust
use eur_lex_loader::loader::load_regulation;
use std::path::Path;

fn main() -> Result<(), eur_lex_loader::error::Error> {
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

Unit tests live alongside their source modules. Integration tests validate the
full parse of four different EU legislative acts against known structural counts:

| File | Act | Articles | Recitals |
|---|---|---|---|
| `tests/eu_ai_act.rs` | EU AI Act (32024R1689) | 113 | 180 |
| `tests/dsa.rs` | Digital Services Act (32022R2065) | 93 | 156 |
| `tests/dsma.rs` | Copyright in the Digital Single Market (32019L0790) | 32 | 86 |
| `tests/trademark_act.rs` | EU Trade Mark Regulation (32017R1001) | 212 | 48 |

---

## Limitations

- Only the English Formex 4 format is tested.
- Footnote bodies (`<NOTE>`) are dropped during text extraction; only the
  surrounding sentence is preserved.
- Formex elements not covered by the model (e.g. tables, images) are silently
  ignored; plain text inside them is still extracted where possible.
