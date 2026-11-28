---
title: How to write a post
pubDate: 2026-11-28
description: The basics of writing a post here, from front matter to Markdown.
---

Post files live in `content/posts/`. The filename becomes the URL:
`content/posts/how-posts-work.md` is served at `/posts/how-posts-work/`.

## Front matter

Every post starts with a small YAML block between `---` lines. It is metadata,
not body text.

```yaml
---
title: How posts work
pubDate: 2026-11-28
description: A short summary used in the post list and link previews.
draft: false
---
```

- `title` — the post heading, browser tab title, and post-list label.
- `pubDate` — the date shown on the page and used for sorting. `date` also works.
- `description` — the summary; falls back to the site description if omitted.
- `draft: true` — skips the post. A filename starting with `_` does the same.

Extra keys such as `author` and `tags` are accepted but not displayed. No
Open Graph image is needed here, so there is no `image` field.

## The body

Everything after the closing `---` is Markdown. A first `#` heading can act as
the title when `title` is omitted, but `##` and `###` headings are the ones that
build the table of contents.

### Text

*Italic*, **bold**, ~~struck through~~, and `inline code`. "Smart quotes",
-- en dashes --- em dashes, and ellipses... are cleaned up automatically.

### Lists

- Unordered item
- Another item
  - Nested item
  - Another nested item

1. Ordered item
2. Second item

- [x] Finished task
- [ ] Open task

### Links

[External link](https://example.com), <https://example.com>, and
[[Markdown specs]] to another post.

### Quotes and rules

> A blockquote.

---

### Code

```rust
fn main() {
    println!("hello from Rust");
}
```

### Math

Inline $E = mc^2$, and display math:

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$

### Footnotes

A note[^one], and the same note again[^one].

[^one]: Click the number and the note opens in a small popover.

## Keep exploring

The [[Markdown specs]] post shows every Markdown feature this site supports.
