# blog

A minimal Rust static site generator built on
[`pulldown-cmark`](https://crates.io/crates/pulldown-cmark). It uses plain HTML
templates and CSS, with layout, colors, and fonts inspired by
[Astro Chiri](https://github.com/the3ash/astro-chiri).

There is no CLI to configure: `cargo run` builds the blog.

## Build

```sh
cargo run
```

This reads `content/`, fills `templates/`, copies `static/`, and writes the site
to `dist/` (gitignored):

```text
site.toml                        Site title, author, description, website, themes, ...
themes/mono.tmTheme              Built-in dark code theme (embedded in the binary)
themes/rose-pine-dawn.tmTheme    Built-in light code theme (embedded in the binary)
content/about.md                 Optional intro on the index page
content/posts/*.md               One page per post at dist/posts/<slug>/index.html
dist/posts/index.html            Archive listing every post
templates/*.html                 Plain templates with {{ key }} / {{{ key }}}
static/styles.css                Chiri-inspired styles; no Tailwind or CSS framework
static/app.js                    Theme, favicon tinting, TOC rail, code copy + View Transitions
static/fonts/                    Inter and Besley, copied from Chiri
static/icons/                    SVG/PNG favicons, apple touch icon, web manifest
static/CNAME                     Custom domain for GitHub Pages
dist/code-theme.css              Generated light/dark code colors for the .hl-* spans
dist/                            Generated output
```

### Site settings

`site.toml` uses simple flat `key = value` lines:

```toml
title = "Chiri-Rust"
author = "Psy"
description = "A minimal static blog generated in Rust."
language = "en-US"
content_width = "35rem"
website = "https://example.com"
theme = "themes/mono.tmTheme"
theme_light = "themes/rose-pine-dawn.tmTheme"
copy_code = true
```

`theme` is the code theme for dark mode and `theme_light` the one for light mode.
Either can be `themes/mono.tmTheme` / `themes/rose-pine-dawn.tmTheme` (both
embedded in the binary), another `.tmTheme` path, or a built-in `syntect` theme
name. The short names (`mono`, `rose-pine-dawn`) still work too. If
`theme_light` is empty, `theme` is used for both modes. The build writes scoped
rules for both themes into `dist/code-theme.css`; toggling the site theme swaps
the code colors instantly without reloading. `copy_code = false` hides the
code-block copy buttons (the equivalent of Chiri's `themeConfig.post.copyCode`).

### Post front matter

Front matter follows Chiri's content schema. Flat values are used for the page
title, date, description, and Open Graph image; nested YAML is ignored:

```markdown
---
title: A post
pubDate: 2025-06-28
description: A short summary.
image: /images/cover.png
---
```

- `title` — post heading, `<title>`, and post-list label.
- `pubDate` (or `date`) — displayed date and sort key, formatted like Chiri
  (`2025.06.28`) and emitted in a `<time datetime="...">` element.
- `description` — page and social description; falls back to the site description.
- `image` — `og:image`/`twitter:image`; falls back to `/og.png`.
- `draft: true` — skips the post. Files starting with `_` are skipped too.

The index page follows Chiri's structure: `.page-content` contains the header
(with the site title and theme toggle), `<main>` with the optional `.about`
block, a plain `<ul>` of up to four `.post-item` rows (`p.title`, a flexible
dotted divider, and `p.date`), a `...` link to `/posts/`, a `.placeholder`, and the
footer. `/posts/` reuses the same list layout to show every post, with a 1.75em
gap above the list instead of a back button. The top gradient mask appears after
scrolling, as in Chiri.

Post pages live at `/posts/<slug>/` and use Chiri's `.post-container` instead:
no site header — the back button to the index (`/`) and table-of-contents rail
take their place — while the footer remains. The theme toggle stays in the same
top-right spot, anchored to the post container.

`content/posts/markdown-specs.md` is the full Markdown showcase;
`content/posts/how-posts-work.md` explains front matter and the basics, and
`content/posts/markdown-test.md` remains the feature test document.

Templates have no loops or conditionals: optional fragments are built in Rust
and passed to `{{{ name }}}` as raw HTML. Unknown keys fail the build instead of
shipping broken pages.

Generated pages load the KaTeX stylesheet from jsDelivr plus a small `/app.js`.
Math is still rendered by Rust; the script only applies the saved/system theme,
handles the header theme toggle, fades in the top gradient mask on scroll,
drives the post table-of-contents rail, cleans up hover states, and uses the
browser's default View Transition between internal links. Footnotes and explicit
heading IDs scroll in place; links that cross a page boundary with a fragment
scroll to that target after the swap instead of jumping to the top. Browsers
without `document.startViewTransition` fall back to a normal navigation, and
`prefers-reduced-motion` disables the animation.

Code blocks are wrapped in Chiri's `.copy-code-wrapper` with a `.copy-button`.
The Rust renderer emits `.hl-*` token spans instead of inline colors, so
`code-theme.css` can restyle them with `html.dark` / `html.light` rules — dark
mode uses `theme` (mono by default), light mode uses `theme_light` (Rose Pine
Dawn). `/app.js` copies the block text to the clipboard, fades the button in on
hover/focus, and shows a check icon for 1.5 seconds after a successful copy.
Buttons are reinitialized after View Transitions, so they keep working on
swapped pages. A subtle `--code-border` outlines each block (`#cecacd` in light
mode, `#333333` in dark mode). The public `render` / `render_with_theme*` helpers
still emit inline colors for single-theme use; the site builder uses
`render_classed`.

The header has a theme toggle with Chiri's two-circle indicator: a hollow
circle and a solid circle side by side. The two circles swap places with the
active theme — filled on the left in light mode, hollow on the left in dark
mode. It stores the choice in `localStorage` under `chiri-theme`, and the inline
head script applies it before CSS loads to avoid a flash. The whole light palette
is Rose Pine Dawn (base `#faf4ed`, text
`#575279`, accents from the same palette); dark mode keeps Chiri's neutral
grays with the mono code theme. The SVG favicon in `static/icons/` follows the
same theme: `/app.js` fetches it once, recolors its paths to `#111`/`#ddd`, and
points the icon link at a blob URL. It re-applies after theme toggles and page
swaps, while the PNG favicon, apple touch icon, and web manifest stay as static
fallbacks.

Footnote definitions are collected and emitted at the very end of the post, no
matter where they are declared in the Markdown, and are hidden as sources for
the popover. Inline footnote references stay where they were written and open a
small themed popover with the note content. No `<hr />` separator is generated.

Posts with h1–h3 headings get Chiri's table of contents on the left: short
horizontal lines that shrink on hover while the heading text fades in beside
them via CSS (`data-text` + `::after`), not a native browser tooltip. The active
section is highlighted while scrolling. It is hidden when there is not enough
side margin or on narrow screens. Explicit `{#id}` heading attributes are
preserved; generated slugs and duplicates follow Chiri's rules.

View transitions fetch pages, so serve `dist/` over HTTP (for example
`python3 -m http.server -d dist`) rather than opening files with `file://`.

## Library

```rust
use blog::render;

let rendered = render("# Hello\n");
println!("{}", rendered.html);
```

`render()` uses the embedded `themes/mono.tmTheme` by default. Use
`render_with_theme_ref()` with `load_theme()` to select another theme.

## Markdown features

All of the requested `pulldown-cmark` options are set in `blog::options()`:

| Feature | Option |
| --- | --- |
| Tables | `ENABLE_TABLES` |
| Footnotes | `ENABLE_FOOTNOTES` |
| Strikethrough | `ENABLE_STRIKETHROUGH` |
| Task lists | `ENABLE_TASKLISTS` |
| Smart punctuation | `ENABLE_SMART_PUNCTUATION` |
| Heading attributes | `ENABLE_HEADING_ATTRIBUTES` |
| YAML-style metadata blocks | `ENABLE_YAML_STYLE_METADATA_BLOCKS` |
| Math | `ENABLE_MATH` |
| Misc GFM (alerts) | `ENABLE_GFM` |
| Definition lists | `ENABLE_DEFINITION_LIST` |
| Wiki links | `ENABLE_WIKILINKS` |

Wikilinks (`[[Some Page]]`, `[[Some Page|label]]`, `[[Some Page#Section]]`) are
slugified to post URLs under `/posts/` (`/posts/some-page/`,
`/posts/some-page/#section`), so they resolve the same from the index and from
nested post pages instead of under the current post's directory.

### Abbreviations

`pulldown-cmark` has no abbreviation option, so this project implements the
Markdown Extra syntax with an event transform:

```markdown
*[HTML]: HyperText Markup Language

HTML and CSS are used on the web.
```

becomes:

```html
<p><abbr title="HyperText Markup Language">HTML</abbr> and CSS are used on the web.</p>
```

Definitions are removed from the output and abbreviations are not expanded
inside code spans, fenced code blocks, math, raw HTML, or YAML front matter.
Longer abbreviations win over shorter ones, and matches inside larger words
(for example `HTML5`) are left alone.

### YAML front matter

A leading metadata block delimited by `---` / `---` (or `---` / `...`) is
captured in `Rendered::metadata` as raw text. The Markdown renderer does not
write it into the body; the site builder turns Chiri-style fields into page
metadata.

### Math and KaTeX

`ENABLE_MATH` produces `Event::InlineMath` and `Event::DisplayMath`. A lazy
`MathRenderer` transform renders both with
[`katex-rs`](https://crates.io/crates/katex-rs) directly in Rust, so the output
is static HTML + MathML with **no browser JavaScript** and no Node/WASM
dependency:

```html
<span class="katex"><span class="katex-mathml"><math>…</math></span>…</span>
<span class="katex-display">…</span>
```

The `KatexContext` is built once with `LazyLock`, and settings are created once
per renderer. Invalid TeX becomes red error text instead of failing the build.

### Syntax highlighting

Fenced and indented code blocks are highlighted with `syntect`. The default is
the embedded `themes/mono.tmTheme`, whose colors appear verbatim in the generated
`<pre style="background-color:...">` and inline `<span style="color:...">`
elements. Unknown languages and indented blocks fall back to plain text.

## Deploy

`.github/workflows/deploy.yml` builds the site with `cargo run` and publishes
`dist/` with the official GitHub Pages actions. It runs on pushes to `master`
or `main`, and can also be started manually from the Actions tab. In the repo
settings, set **Pages → Build and deployment → Source** to **GitHub Actions**.

`static/CNAME` carries the custom domain from `site.toml` (`psyny.de`) into the
published site. Remove that file if you want to use the default
`<user>.github.io` address instead.

## Tests

```sh
cargo test
```
