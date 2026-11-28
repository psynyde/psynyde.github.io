use std::path::Path;

use blog::{
    code_theme_css, load_theme, render, render_classed, render_with_theme, render_with_theme_ref,
};

#[test]
fn renders_requested_extensions() {
    let markdown = r#"---
title: Example
---

# Title { #main .lead }

| A | B |
| - | - |
| 1 | 2 |

A note[^note] with ~~removed~~ and "quotes".

- [ ] open
- [x] done

Term
: Definition

$inline$ and $$display$$

> [!NOTE]
> Be aware.

[[Wiki Link]]

[^note]: The footnote.

*[HTML]: HyperText Markup Language

HTML is everywhere.
"#;

    let rendered = render(markdown);
    let html = &rendered.html;

    // Heading attributes.
    assert!(html.contains("<h1 id=\"main\" class=\"lead\">Title</h1>"));

    // Tables.
    assert!(html.contains("<table>"));
    assert!(html.contains("<th>A</th>"));

    // Footnotes are collected at the end without a redundant rule, not left
    // where they were declared (just after the wiki link, before the
    // abbreviation).
    assert!(html.contains("class=\"footnote-reference\""));
    assert!(html.contains("class=\"footnote-definition\""));
    assert!(!html.contains("<hr />"), "no footnote separator rule");
    let note = html
        .rfind("class=\"footnote-definition\"")
        .expect("footnote definition");
    let body_end = html.find("everywhere.").expect("last body text");
    assert!(note > body_end, "the notes must come after the body");

    // Strikethrough.
    assert!(html.contains("<del>removed</del>"));

    // Task lists.
    assert_eq!(html.matches("type=\"checkbox\"").count(), 2);

    // Smart punctuation.
    assert!(html.contains("“quotes”"));

    // Definition lists.
    assert!(html.contains("<dl>"));
    assert!(html.contains("<dt>Term</dt>"));
    assert!(html.contains("<dd>Definition</dd>"));

    // Math is rendered server-side by katex-rs.
    assert!(html.contains("<span class=\"katex\">"));
    assert!(html.contains("<span class=\"katex-display\">"));
    assert!(html.contains("<annotation encoding=\"application/x-tex\">inline</annotation>"));
    assert!(html.contains("<annotation encoding=\"application/x-tex\">display</annotation>"));
    assert!(!html.contains("class=\"math"));

    // GFM alerts.
    assert!(html.contains("markdown-alert-note"));

    // Wiki links.
    assert!(html.contains("<a href="));
    assert!(html.contains("Wiki Link</a>"));

    // Abbreviations.
    assert!(html.contains("<abbr title=\"HyperText Markup Language\">HTML</abbr>"));

    // YAML front matter is captured, not rendered into the body.
    let metadata = rendered
        .metadata
        .as_deref()
        .expect("metadata should be captured");
    assert!(metadata.contains("title: Example"));
    assert!(!html.contains("title: Example"));
    assert!(!html.contains("*[HTML]:"));
}

#[test]
fn abbreviations_respect_word_boundaries_and_code_blocks() {
    let markdown = r#"* [HTML]: HyperText Markup Language

HTML, HTML5 and `HTML`.

~~~
HTML
~~~

Use HTML again.
"#;

    // The definition above is intentionally split (`* [HTML]`) so it is not a
    // definition; use the real syntax below instead.
    assert!(!render(markdown).html.contains("<abbr"));

    let markdown = r#"*[HTML]: HyperText Markup Language

HTML, HTML5 and `HTML`.

~~~
HTML
~~~

Use HTML again.
"#;

    let html = render(markdown).html;
    assert_eq!(html.matches("<abbr ").count(), 2);
    assert!(html.contains("<abbr title=\"HyperText Markup Language\">HTML</abbr>"));
}

#[test]
fn metadata_block_can_use_three_dots_as_terminator() {
    let rendered = render("---\ntitle: Ended\n...\n\nBody\n");
    assert_eq!(rendered.metadata.as_deref(), Some("title: Ended\n"));
    assert!(rendered.html.contains("<p>Body</p>"));
}

#[test]
fn code_blocks_are_highlighted_with_syntect() {
    let html = render("```rust\nfn main() {}\n```\n").html;

    assert!(html.contains("class=\"copy-code-wrapper\""));
    assert!(html.contains(
        "<button class=\"copy-button\" type=\"button\" aria-label=\"Copy code to clipboard\"></button>"
    ));
    assert!(html.contains("<pre class=\"copy-code-block\" style=\"background-color:"));
    assert!(html.contains("<span style=\""));
    assert!(html.contains("fn"));
    assert!(html.contains("</pre></div>"));
    assert!(!html.contains("<pre><code"));
}

#[test]
fn unknown_languages_and_indented_blocks_fall_back_to_plain_text() {
    let fenced = render("```not-a-real-language\nhello\n```\n").html;
    assert!(fenced.contains("class=\"copy-code-wrapper\""));
    assert!(fenced.contains("<pre class=\"copy-code-block\""));
    assert!(fenced.contains("hello"));

    let indented = render("    indented code\n").html;
    assert!(indented.contains("class=\"copy-code-wrapper\""));
    assert!(indented.contains("<pre class=\"copy-code-block\""));
    assert!(indented.contains("indented code"));
}

#[test]
fn unknown_theme_falls_back_to_default() {
    let markdown = "```rust\nfn main() {}\n```\n";
    assert_eq!(
        render_with_theme(markdown, "does-not-exist").html,
        render(markdown).html
    );
}

#[test]
fn mono_tm_theme_is_used_by_default() {
    let markdown = "```rust\nfn main() {}\n```\n";
    let theme = load_theme(Path::new("themes/mono.tmTheme")).expect("mono.tmTheme should load");
    let html = render_with_theme_ref(markdown, &theme).html;

    assert_eq!(html, render(markdown).html);
    assert!(html.contains("<pre class=\"copy-code-block\" style=\"background-color:#101010;\">"));
    assert!(html.contains("color:#6c6c6c")); // keyword
    assert!(html.contains("color:#626262")); // function name
}

#[test]
fn classed_code_blocks_use_tokens_instead_of_inline_colors() {
    let html = render_classed("```rust\nfn main() {}\n```\n").html;

    assert!(html.contains("<pre class=\"copy-code-block hl-code\">"));
    assert!(html.contains("class=\"hl-"));
    assert!(!html.contains("<span style="));
    assert!(!html.contains("style=\"background-color:"));
    assert!(html.contains("class=\"copy-code-wrapper\""));
    assert!(html.contains("fn"));
}

#[test]
fn code_theme_css_is_scoped_to_a_selector() {
    let mono = load_theme(Path::new("themes/mono.tmTheme")).unwrap();
    let css = code_theme_css(&mono, "html.dark").unwrap();

    assert!(css.contains("html.dark .hl-code"));
    assert!(css.contains(" background-color: #101010;"));
    assert!(!css.contains("\n.hl-code {"));
}

#[test]
fn math_is_rendered_server_side_with_katex() {
    let html = render("Inline $x^2$ and display:\n\n$$\\frac{1}{2}$$\n").html;

    assert!(html.contains("<span class=\"katex\">"));
    assert!(html.contains("<span class=\"katex-display\">"));
    assert!(html.contains("<math"));
    assert!(html.contains("<annotation encoding=\"application/x-tex\">x^2</annotation>"));
    assert!(html.contains("<annotation encoding=\"application/x-tex\">\\frac{1}{2}</annotation>"));
    assert!(!html.contains("math-inline"));
    assert!(!html.contains("math-display"));

    // Invalid TeX is rendered as red error text rather than failing the render.
    let bad = render("$\\unknowncmd$\n").html;
    assert!(bad.contains("#cc0000"));
    assert!(bad.contains("\\unknowncmd"));
}

#[test]
fn wikilinks_support_plain_alias_and_section_forms() {
    let html = render(
        "[[Some Page]] and [[Another Page|with custom label]] and [[Target Page#Section]].\n",
    )
    .html;

    assert!(html.contains("<a href=\"/posts/some-page/\">Some Page</a>"));
    assert!(html.contains("<a href=\"/posts/another-page/\">with custom label</a>"));
    assert!(html.contains("<a href=\"/posts/target-page/#section\">Target Page#Section</a>"));
}

#[test]
fn toc_collects_headings_and_adds_anchors() {
    let rendered = render(
        "# Post title\n\n## Hello, World!\n\n### Hello, World!\n\n## Custom {#mine}\n\n## 中文标题\n",
    );

    assert_eq!(rendered.toc.len(), 4);
    assert_eq!(rendered.toc[0].level, 2);
    assert_eq!(rendered.toc[0].text, "Hello, World!");
    assert_eq!(rendered.toc[0].id, "hello-world");
    assert_eq!(rendered.toc[1].id, "hello-world-1");
    assert_eq!(rendered.toc[2].id, "mine");
    assert_eq!(rendered.toc[3].id, "中文标题");

    assert!(rendered.html.contains("<h1>Post title</h1>"));
    assert!(
        rendered
            .html
            .contains("<h2 id=\"hello-world\">Hello, World!</h2>")
    );
    assert!(
        rendered
            .html
            .contains("<h3 id=\"hello-world-1\">Hello, World!</h3>")
    );
    assert!(rendered.html.contains("<h2 id=\"mine\">Custom</h2>"));
    assert!(rendered.html.contains("<h2 id=\"中文标题\">中文标题</h2>"));
}
