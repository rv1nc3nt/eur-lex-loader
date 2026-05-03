# eur-lex-loader

A Rust crate for working with EU legislative acts published in
[Formex 4](https://op.europa.eu/en/web/eu-vocabularies/formex) XML format.
It provides two command-line tools and a library:

- **`eur_lex_fetch`** — downloads a Formex publication from the EUR-Lex Cellar
  repository by CELEX number and extracts it into a local directory.
- **`eur_lex_loader`** — parses a local Formex directory and converts the act to
  JSON, or fetches and converts in a single step.
- **Library** — exposes `load_act` and the full data model for embedding in Rust
  applications. The public API is documented with `cargo doc --open`.

The library extracts the full document structure: bibliographic metadata (CELEX
number, document date, legal value, Official Journal reference, authors),
title, preamble (legal bases and recitals), enacting terms (chapters, sections,
articles, and nested lists), tables, annexes, and a flat definitions map when
the act contains a Definitions article. Both original and consolidated acts are
supported.

---

## European legislative acts

The European Union produces two main types of binding secondary legislation.

**Regulations** are directly applicable across all member states from the
moment they enter into force. No national transposition is needed; a regulation
has the same legal force as national law in every member state the day it is
published in the Official Journal.

**Directives** are binding as to the result to be achieved but leave each
member state free to choose the form and methods. They must be transposed into
national law within a deadline set by the directive itself. The national
transposition laws differ from country to country, but the outcome must meet
the directive's requirements.

**Consolidated versions** are unofficial editorial compilations produced by the
Publications Office. They integrate all subsequent amendments into the original
text so that readers see the current wording in a single document, without
having to cross-reference a chain of amending acts. Consolidated versions have
no independent legal force — only the original act and its amending acts are
legally binding — but they are the most convenient starting point for reading
the current state of a piece of legislation.

---

## EUR-Lex, Cellar, and CELEX numbers

[EUR-Lex](https://eur-lex.europa.eu) is the official portal for EU law,
providing free access to the Official Journal and to the full text of all EU
legislative acts.

The [Cellar](https://op.europa.eu/en/web/cellar) content repository, maintained
by the Publications Office of the European Union, is the underlying store from
which EUR-Lex serves its content. Formex XML files are available directly from
Cellar without authentication.

### CELEX numbers

Every EU legal act has a unique CELEX identifier. The format is:

```
3 YYYY T NNNN
│  │   │  └─ sequential number within the year
│  │   └─ document type: R = Regulation, L = Directive
│  └─ year of publication
└─ sector: 3 = secondary legislation
```

Examples — all six acts included as test fixtures in this repository:

| Act | CELEX | Format |
|---|---|---|
| EU AI Act (2024) | `32024R1689` | Original regulation |
| Digital Services Act (2022) | `32022R2065` | Original regulation |
| EU Trade Mark Regulation (2017) | `32017R1001` | Original regulation |
| Copyright in the Digital Single Market Directive (2019) | `32019L0790` | Original directive |
| REACH Regulation (2006) | `32006R1907` | Consolidated regulation |
| Consumer Rights Directive (2011) | `32011L0083` | Consolidated directive |

The CELEX number appears in every EUR-Lex URL, e.g.:
`https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689`

---

## Building

```bash
cargo build --release
```

Two binaries are produced under `target/release/`: `eur_lex_fetch` and
`eur_lex_loader`.

---

## Fetching an act — `eur_lex_fetch`

Downloads a Formex ZIP archive from the EUR-Lex Cellar API by CELEX number,
extracts it into a local directory, then prints the act title to stdout so you
can confirm the correct act was retrieved. Progress messages go to stderr.

```
eur_lex_fetch [OPTIONS] <CELEX> <DIR>

Arguments:
  <CELEX>  CELEX number of the act to fetch (e.g. 32024R1689)
  <DIR>    Directory where the Formex files will be extracted

Options:
  -l, --lang <LANG>  Language code (ISO 639-2/B, e.g. eng, fra, deu) [default: eng]
  -h, --help         Print help
  -V, --version      Print version
```

```bash
# Fetch the EU AI Act in English
eur_lex_fetch 32024R1689 data/32024R1689
# → Fetching 32024R1689 (eng)...
# → Extracted to data/32024R1689
# → Regulation (EU) 2024/1689 of the European Parliament …

# Fetch the REACH Regulation in French
eur_lex_fetch 32006R1907 data/32006R1907_fr --lang fra
```

The extracted directory will contain several `.fmx.xml` files:

| Filename pattern | Content |
|---|---|
| `*.000101.fmx.xml` | Main act (title, preamble, enacting terms) |
| `*.012401.fmx.xml` and above | Annexes, one file each (original acts) |
| `*.doc.fmx.xml` | Registry listing all files in order |
| `*.toc.fmx.xml` | Table of contents (not used by this tool) |

Consolidated acts embed their annexes inline in the main file; no separate
annex files are produced.

> **Rate limiting**: keep concurrent requests below 5 per IP address.

---

## Converting to JSON — `eur_lex_loader`

Parses a local Formex directory (previously fetched with `eur_lex_fetch` or
downloaded manually) and writes the act as JSON to stdout or a file. Can also
fetch directly from Cellar without saving the Formex files locally.

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

```bash
# Fetch the DSA directly from EUR-Lex and pretty-print to stdout
eur_lex_loader -c 32022R2065

# Fetch the EU AI Act and write compact JSON to a file
eur_lex_loader -c 32024R1689 --compact --output ai_act.json

# Parse a previously downloaded act
eur_lex_loader data/32024R1689

# Write compact JSON to a file
eur_lex_loader data/32024R1689 --compact --output ai_act.json

# Pipe pretty-printed JSON into jq
eur_lex_loader data/32024R1689 | jq '.preamble.recitals | length'
```

### Output format

The tool outputs a single JSON object. The shape depends on whether the act is
an original or a consolidated version.

**Original acts** include a full preamble:

```jsonc
{
  "metadata": {
    "celex": "32024R1689",
    "document_date": "20240613",
    "legal_value": "REG",
    "language": "EN",
    "authors": ["PE", "CS"],
    "eea_relevant": true,
    "official_journal": { "collection": "L", "number": "1689", "date": "20240712", "language": "EN" },
    "page_first": 1,
    "page_last": 144,
    "page_total": 144
  },

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

  "enacting_terms": { "…": "…" },
  "annexes": [ "…" ],
  "definitions": { "…": "…" }
}
```

**Consolidated acts** have a slim preamble with no visas or recitals:

```jsonc
{
  "metadata": { "celex": "32006R1907", "legal_value": "REG", "…": "…" },

  "title": "Regulation (EC) No 1907/2006 …",

  "preamble": {
    "init": "THE EUROPEAN PARLIAMENT AND THE COUNCIL …",
    "enacting_formula": "HAVE ADOPTED THIS REGULATION:"
  },

  "enacting_terms": { "…": "…" },
  "annexes": [ "…" ]
}
```

**Full output shape:**

```jsonc
{
  "metadata": {
    "celex": "32024R1689",          // CELEX identifier
    "document_date": "20240613",    // signing/adoption date, YYYYMMDD
    "legal_value": "REG",           // "REG" | "DIR" | "DEC" | …
    "language": "EN",               // document language code
    "authors": ["PE", "CS"],        // institutional authors
    "eea_relevant": true,           // EEA relevance flag
    "official_journal": {
      "collection": "L",            // OJ series ("L" or "C")
      "number": "1689",             // OJ issue number
      "date": "20240712",           // publication date, YYYYMMDD
      "language": "EN"              // language edition
    },
    "page_first": 1,
    "page_last": 144,
    "page_total": 144,
    "prod_id": "20240610001",       // internal production ID (absent in older acts)
    "fin_id": "789012"              // internal final ID (absent in older acts)
  },

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
                    } },
                    // A table parsed from <GR.TBL> or a bare <TBL> element:
                    { "Table": {
                        "col_count": 3,
                        "title": "Correlation table",   // omitted when absent
                        "row_count": 2,
                        "rows": [
                          { "is_header": true, "cell_count": 3,
                            "cells": [
                              { "text": "Old directive", "is_header": true },
                              { "text": "New directive", "is_header": true },
                              { "text": "Remarks",       "is_header": true }
                            ] },
                          { "cell_count": 3,
                            "cells": [
                              { "text": "Article 1" },
                              { "text": "Article 3" },
                              { "text": "" }
                            ] }
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
              ] } },
              { "Table": { "col_count": 2, "row_count": 1,
                           "rows": [ { "cell_count": 2,
                                       "cells": [ { "text": "…" }, { "text": "…" } ] } ] } }
            ]
          }
        ]
      }
      // Annexes with flat numbered items, plain text, or tables use Paragraphs:
      // "content": {
      //   "Paragraphs": [
      //     { "number": "1.", "alineas": [ { "Text": { "text": "…" } } ] },
      //     { "number": "2.", "alineas": [ { "List": { "intro": "…", "items": [ "…" ] } } ] },
      //     { "number": null, "alineas": [ { "Table": { "col_count": 3, "row_count": 5, "rows": [ "…" ] } } ] }
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
lists). `title` is omitted from `Table` when the `<TBL>` element has no
`<TITLE>`. `is_header` is omitted from `Row` and `Cell` when `false`.
`definitions` is omitted when the act has no Definitions article.

In `metadata`, all fields except `eea_relevant` are optional and omitted from
the JSON when absent. `prod_id` and `fin_id` are absent in older Formex files.
`authors` is omitted when empty.

---

## Running the tests

```bash
cargo test
```

Unit tests live alongside their source modules. Integration tests validate the
full parse of six different EU legislative acts against known structural counts:

| File | Act | Format | Articles | Recitals | Definitions | Tables |
|---|---|---|---|---|---|---|
| `tests/eu_ai_act.rs` | EU AI Act (`data/32024R1689`) | Original | 113 | 180 | 68 | — |
| `tests/dsa.rs` | Digital Services Act (`data/32022R2065`) | Original | 93 | 156 | 27 | — |
| `tests/dsma.rs` | Copyright in the Digital Single Market (`data/32019L0790`) | Original | 32 | 86 | 6 | — |
| `tests/trademark_act.rs` | EU Trade Mark Regulation (`data/32017R1001`) | Original | 212 | 48 | — | — |
| `tests/reach.rs` | REACH Regulation (`data/32006R1907`) | Consolidated | 141 | — | — | ✓ |
| `tests/consumer_directive.rs` | Consumer Rights Directive (`data/32011L0083`) | Consolidated | 36 | — | — | ✓ |

The table tests (✓) verify that `Subparagraph::Table` values are produced for
annex tables in both Formex table encodings:

- **REACH** (ANNEX IV) — a bare `<TBL>` element sitting directly inside a
  `<CONTENTS>` block, with no wrapping `<GR.TBL>`.
- **Consumer Rights Directive** (ANNEX II) — a correlation table wrapped in a
  `<GR.TBL>` element, which carries an optional title above the table.

The integration tests require the Formex data to be present in the `data/`
directory. All six fixtures are included in the repository.

---

## Limitations

- Only the English Formex 4 format is tested.
- Footnote bodies (`<NOTE>`) are dropped during text extraction; only the
  surrounding sentence is preserved.
- Formex elements not covered by the model (e.g. images, mathematical formulae)
  are silently reduced to their plain-text content where possible; structure is
  lost.
