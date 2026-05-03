# Checks to be done

## `number` as `Option<u32>` in `Subparagraph::Text` and `ListBlock`

`number` is `Option` because `Subparagraph::Text` serves two distinct roles:

1. **Plain text block** — a paragraph of text that is not a list item. These have no position. `number: None`.
2. **List item** — text that is one entry in a `ListBlock`. These always have a position. `number: Some(pos)`.

The `None` is not "we don't know the position" — it's "this text is not a list item at all". The same type covers both cases, so the option is load-bearing.

Same logic for `ListBlock.number`: a top-level list has no position in a parent (`None`), while a nested list is itself item N in its parent (`Some(pos)`).

The alternative that would let you drop the `Option` is to split `Subparagraph::Text` into two variants:

```rust
enum Subparagraph {
    Text(String),                          // plain text, never numbered
    Item { position: u32, text: String },  // list item, always numbered
    List(ListBlock),
    Table(Table),
}
```

And similarly remove `number` from `ListBlock` by making a list-that-is-also-an-item a distinct thing. But that's a structural refactor. The current design keeps `Option<u32>` as the cheaper way to encode the distinction without adding variants.

### Concrete examples from EU AI Act, Article 5, paragraph 1

The Formex XML for this paragraph is (abridged):

```xml
<PARAG>
  <NO.PARAG>1.</NO.PARAG>
  <ALINEA>
    <P>The following AI practices shall be prohibited:</P>
    <LIST TYPE="alpha">
      <ITEM><NP><NO.P>(a)</NO.P><TXT>…subliminal techniques…</TXT></NP></ITEM>
      <ITEM><NP><NO.P>(b)</NO.P><TXT>…vulnerabilities…</TXT></NP></ITEM>
      <ITEM><NP><NO.P>(c)</NO.P><TXT>…social scoring…leading to either or both of the following:</TXT>
        <P><LIST TYPE="roman">
          <ITEM><NP><NO.P>(i)</NO.P><TXT>detrimental treatment in unrelated contexts;</TXT></NP></ITEM>
          <ITEM><NP><NO.P>(ii)</NO.P><TXT>unjustified treatment;</TXT></NP></ITEM>
        </LIST></P>
      </NP></ITEM>
      <!-- …items (d) through (h)… -->
    </LIST>
  </ALINEA>
  <ALINEA>The use of AI systems referred to in points (d), (e) and (f)…</ALINEA>
</PARAG>
```

This produces the following `Paragraph`:

```rust
Paragraph {
    number: Some("1."),
    alineas: vec![
        // First ALINEA: the <P> intro becomes the list's intro field;
        // the whole thing is one List block.
        Subparagraph::List(ListBlock {
            list_type: Some(ListType::Alpha), // TYPE="alpha"
            number: None,                     // top-level list: no position in a parent
            intro: "The following AI practices shall be prohibited:",
            items: vec![
                // Item (a): simple text item, first in the list.
                Subparagraph::Text { number: Some(1), text: "…subliminal techniques…" },

                // Item (b): simple text item, second in the list.
                Subparagraph::Text { number: Some(2), text: "…vulnerabilities…" },

                // Item (c): has a nested sub-list, so it becomes a ListBlock.
                // Its number (Some(3)) is its position in the parent alpha list.
                Subparagraph::List(ListBlock {
                    list_type: Some(ListType::Roman), // TYPE="roman"
                    number: Some(3),                  // 3rd item in the parent alpha list
                    intro: "…social scoring…",
                    items: vec![
                        // Sub-item (i): first item of the roman sub-list.
                        Subparagraph::Text { number: Some(1), text: "detrimental treatment…" },
                        // Sub-item (ii): second item.
                        Subparagraph::Text { number: Some(2), text: "unjustified treatment…" },
                    ],
                }),

                // …items (d) through (h)…
            ],
        }),

        // Second ALINEA: a plain trailing sentence, not a list item.
        Subparagraph::Text {
            number: None, // not a list item — no position
            text: "The use of AI systems referred to in points (d), (e) and (f)…",
        },
    ],
    citations: vec![],
}
```

The four cases in one paragraph:
| Value | Meaning |
|---|---|
| `Subparagraph::Text { number: None, .. }` | Plain text — the trailing sentence of the second alinea |
| `Subparagraph::Text { number: Some(1), .. }` | List item — item (a), first position in the alpha list |
| `ListBlock { number: None, .. }` | Top-level list — the alpha list itself, not inside a parent item |
| `ListBlock { number: Some(3), .. }` | Nested list — item (c)'s roman sub-list, third position in the alpha list |
