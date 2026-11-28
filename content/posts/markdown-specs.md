---
title: Markdown specs
pubDate: 2026-11-28
description: Every Markdown feature this site supports, in one page.
---

This page is a showcase of the Markdown extensions enabled by the renderer.
The front matter above is metadata; everything below is body content.

## Headings

Use `#` through `######`. The first `#` is treated as the post title and skipped
in the table of contents, so `##` and `###` headings make up the rail.

### A nested heading {#custom-heading}

An explicit ID lets you link straight to it:
[jump to custom-heading](#custom-heading).

## Paragraphs and emphasis

*Italic*, **bold**, ***both***, ~~struck through~~, and `inline code`.
"Smart quotes", -- en dashes --- em dashes, and ellipses... are cleaned up.

## Lists

- Unordered item
- Another item
  - Nested item
  - Another nested item

1. Ordered item
2. Second item

- [x] Finished task
- [ ] Open task
  - [x] Nested finished task

## Links

[Standard link](https://example.com), <https://example.com>, and
<test@example.com>.

[[How posts work]] links to another post, while
[[How posts work|with a custom label]] uses different text.

## Blockquotes and alerts

> A plain blockquote with **bold** and *italic* text.

> [!NOTE]
> GitHub-style alerts are recognized too.

## Tables

| Feature | Status | Notes |
|:--------|:------:|------:|
| Tables | yes | aligned columns |
| Code | yes | `inline` and fenced |
| Math | yes | $x^2$ |

## Code blocks

```rust
fn main() {
    println!("hello from Rust");
}
```

```python
def greet(name):
    print(f"hello, {name}")
```

```
Plain, unhighlighted block.
```

## Math

Inline: $E = mc^2$ and $a^2 + b^2 = c^2$.

Display:

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$

$$
\begin{pmatrix} a & b \\ c & d \end{pmatrix}
$$

Math is rendered on the server; the browser only loads the KaTeX stylesheet.

## Footnotes

A first note[^one], a named note[^detail], and the first one reused[^one].

[^one]: The default label is the footnote number.
[^detail]: Named notes can contain **formatting**, *emphasis*, and math $y = mx + b$.

## Definition lists

HTML
: HyperText Markup Language, the format of web pages.

CSS
: Cascading Style Sheets, used for styling web pages.

## Abbreviations

*[HTML]: HyperText Markup Language

HTML expands on first use, but `HTML` in code and HTML5 stay untouched.

## Combined

- A task with math:
  - [x] Solved $x^2 = 4$
- Table cell with footnote[^one]
- ~~Struck~~ text with math: $E = mc^2$

> Blockquote with **bold**, ~~strike~~, and a footnote[^detail].

Final line with "quotes", ~~strike~~, $x^2$, and [[How posts work]].
