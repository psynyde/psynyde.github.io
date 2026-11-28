//! Markdown-to-HTML rendering for the blog.
//!
//! `pulldown-cmark` handles the CommonMark parsing and most extensions. Several
//! transforms are applied at the event layer:
//!
//! - YAML-style front matter is captured into [`Rendered::metadata`] instead of
//!   being written into the HTML body.
//! - Abbreviations (`*[HTML]: HyperText Markup Language`) are expanded into
//!   `<abbr>` elements. `pulldown-cmark` has no built-in abbreviation option.
//! - Math events are rendered server-side with `katex-rs` into KaTeX HTML +
//!   MathML.
//! - Heading anchors are generated and collected into [`TocItem`] entries for
//!   Chiri-style post tables of contents.
//! - Footnote definitions are moved to the end of the document behind a rule.
//!
//! Fenced and indented code blocks are highlighted with `syntect` and emitted
//! as ready-to-use HTML. Every transform is a lazy iterator.

#![forbid(unsafe_code)]

pub mod site;
pub mod template;

use std::collections::{HashSet, VecDeque};
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use katex::{KatexContext, Settings, render_to_string};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd, html,
};
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{
    ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style, highlighted_html_for_string,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Default `.tmTheme` file used by the blog builder and [`render`].
pub const DEFAULT_THEME_FILE: &str = "mono.tmTheme";

/// Prefix for the token classes emitted by [`render_classed`]. Keeping them
/// separate from site classes lets a generated stylesheet own code colors.
const CODE_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

/// Button injected into every rendered code block, matching Chiri's copy UI.
const COPY_CODE_BUTTON: &str =
    "<button class=\"copy-button\" type=\"button\" aria-label=\"Copy code to clipboard\"></button>";

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);
static MONO_THEME: LazyLock<Theme> = LazyLock::new(|| {
    let mut reader = std::io::Cursor::new(include_bytes!("../themes/mono.tmTheme"));
    ThemeSet::load_from_reader(&mut reader).expect("mono.tmTheme should be valid")
});
static ROSE_PINE_DAWN_THEME: LazyLock<Theme> = LazyLock::new(|| {
    let mut reader = std::io::Cursor::new(include_bytes!("../themes/rose-pine-dawn.tmTheme"));
    ThemeSet::load_from_reader(&mut reader).expect("rose-pine-dawn.tmTheme should be valid")
});
static KATEX_CONTEXT: LazyLock<KatexContext> = LazyLock::new(KatexContext::default);

/// All requested `pulldown-cmark` extensions.
///
/// `ENABLE_OLD_FOOTNOTES`, `ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS`, and
/// `ENABLE_ABBR` are deliberately not used: the first two were not requested,
/// and the third does not exist in `pulldown-cmark`.
pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_WIKILINKS
}

/// An abbreviation definition, e.g. `*[HTML]: HyperText Markup Language`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abbreviation {
    pub short: String,
    pub title: String,
}

/// A heading used by the post table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocItem {
    /// Heading level (`1`–`3`); deeper headings are not included.
    pub level: u8,
    /// Plain-text heading label.
    pub text: String,
    /// Generated or explicit heading anchor.
    pub id: String,
}

/// The result of rendering a Markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// HTML body fragment.
    pub html: String,
    /// Raw YAML front matter, if the document had a metadata block.
    pub metadata: Option<String>,
    /// Abbreviations found in the document, longest first.
    pub abbreviations: Vec<Abbreviation>,
    /// Chiri-style table of contents entries for h1–h3 headings.
    pub toc: Vec<TocItem>,
}

/// Render Markdown to an HTML fragment using all configured extensions and the
/// embedded mono theme for code blocks.
pub fn render(markdown: &str) -> Rendered {
    render_with_theme_ref(markdown, &MONO_THEME)
}

/// Render Markdown with a built-in `syntect` theme selected by name.
///
/// Unknown theme names fall back to the embedded mono theme. Use
/// [`render_with_theme_ref`] with a theme from [`load_theme`] for a custom
/// `.tmTheme` file.
pub fn render_with_theme(markdown: &str, theme: &str) -> Rendered {
    render_with_theme_ref(markdown, theme_by_name(theme))
}

/// Render Markdown with a specific `syntect` [`Theme`].
///
/// Syntax sets and the built-in theme set are loaded once via [`LazyLock`], and
/// all event transforms are lazy iterators.
pub fn render_with_theme_ref(markdown: &str, theme: &Theme) -> Rendered {
    render_inner(markdown, Some(theme))
}

/// Render Markdown with theme-independent code spans (`.hl-*` classes).
///
/// Colors come from [`code_theme_css`], so switching the site's light/dark class
/// also switches the code theme without re-rendering any HTML.
pub fn render_classed(markdown: &str) -> Rendered {
    render_inner(markdown, None)
}

fn render_inner(markdown: &str, theme: Option<&Theme>) -> Rendered {
    let (markdown, abbreviations) = extract_abbreviations(markdown);

    let mut metadata = None;
    let parser = Parser::new_ext(&markdown, options());
    let events = MetadataCapture::new(parser, &mut metadata);
    let events = AbbreviationExpander::new(events, &abbreviations);
    let events = MathRenderer::new(events);
    let events = WikiLinkRoots::new(events);
    let events = CodeHighlighter::new(events, theme);

    let mut toc = Vec::new();
    let events = HeadingIds::new(events, &mut toc);
    let events = FootnotesToEnd::new(events);

    let mut html = String::with_capacity(markdown.len() * 3 / 2);
    html::push_html(&mut html, events);

    Rendered {
        html,
        metadata,
        abbreviations,
        toc,
    }
}

/// Generate CSS for a code theme, scoped under a selector like `html.dark`.
///
/// The returned rules use the same `.hl-*` classes emitted by
/// [`render_classed`], so one stylesheet can hold a light and a dark theme.
pub fn code_theme_css(theme: &Theme, scope: &str) -> io::Result<String> {
    let css = css_for_theme_with_class_style(theme, CODE_CLASS_STYLE)
        .map_err(|error| io::Error::other(format!("failed to generate code theme CSS: {error}")))?;

    let mut scoped = String::with_capacity(css.len() + scope.len());
    for line in css.lines() {
        if line.ends_with(" {") && !line.starts_with(char::is_whitespace) {
            scoped.push_str(scope);
            scoped.push(' ');
        }
        scoped.push_str(line);
        scoped.push('\n');
    }
    Ok(scoped)
}

/// Look up a built-in `syntect` theme by name, falling back to mono.
pub fn theme_by_name(name: &str) -> &'static Theme {
    THEME_SET.themes.get(name).unwrap_or(&MONO_THEME)
}

/// Load a TextMate `.tmTheme` file as a `syntect` [`Theme`].
///
/// This uses syntect's `plist-load` feature, enabled by `default-fancy`.
pub fn load_theme(path: &Path) -> io::Result<Theme> {
    ThemeSet::get_theme(path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to load theme {}: {error}", path.display()),
        )
    })
}

/// Resolve a theme name or `.tmTheme` path from site configuration.
///
/// `mono`, `mono.tmTheme`, and `themes/mono.tmTheme` use the embedded default;
/// the same applies to `rose-pine-dawn` and its `.tmTheme` paths. Other paths
/// are loaded from disk; anything else is looked up in syntect's built-in
/// themes.
pub fn theme_for(name_or_path: &str) -> io::Result<Theme> {
    if name_or_path.eq_ignore_ascii_case("mono")
        || name_or_path == DEFAULT_THEME_FILE
        || name_or_path == "themes/mono.tmTheme"
    {
        return Ok(MONO_THEME.clone());
    }
    if name_or_path.eq_ignore_ascii_case("rose-pine-dawn")
        || name_or_path == "rose-pine-dawn.tmTheme"
        || name_or_path == "themes/rose-pine-dawn.tmTheme"
    {
        return Ok(ROSE_PINE_DAWN_THEME.clone());
    }

    let path = Path::new(name_or_path);
    if path
        .extension()
        .is_some_and(|extension| extension == "tmTheme")
        || path.components().count() > 1
    {
        return load_theme(path);
    }

    Ok(theme_by_name(name_or_path).clone())
}

/// Assign stable anchors to h1–h3 headings and collect TOC entries.
///
/// This mirrors Chiri's `remark-toc` plugin: the first h1 is treated as the post
/// title and skipped, slugs come from plain heading text, and duplicate slugs
/// get `-1`, `-2`, ... suffixes. Explicit `{#id}` attributes are preserved.
struct HeadingIds<'a, 'o, I> {
    inner: I,
    toc: &'o mut Vec<TocItem>,
    used_ids: HashSet<String>,
    heading_index: usize,
    pending: Option<PendingHeading<'a>>,
    queue: VecDeque<Event<'a>>,
}

struct PendingHeading<'a> {
    id: Option<CowStr<'a>>,
    classes: Vec<CowStr<'a>>,
    attrs: Vec<(CowStr<'a>, Option<CowStr<'a>>)>,
    text: String,
    events: Vec<Event<'a>>,
}

impl<'a, 'o, I> HeadingIds<'a, 'o, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn new(inner: I, toc: &'o mut Vec<TocItem>) -> Self {
        Self {
            inner,
            toc,
            used_ids: HashSet::new(),
            heading_index: 0,
            pending: None,
            queue: VecDeque::new(),
        }
    }

    fn flush(&mut self, level: HeadingLevel) -> Option<Event<'a>> {
        let pending = self.pending.take()?;
        let text = pending.text.trim().to_owned();
        let id = match pending.id {
            Some(id) if !id.is_empty() => unique_id(id.to_string(), &mut self.used_ids),
            _ => unique_id(slugify(&text), &mut self.used_ids),
        };

        if !text.is_empty() {
            self.toc.push(TocItem {
                level: level as u8,
                text,
                id: id.clone(),
            });
        }
        self.heading_index += 1;

        self.queue.extend(pending.events);
        self.queue.push_back(Event::End(TagEnd::Heading(level)));

        Some(Event::Start(Tag::Heading {
            level,
            id: Some(CowStr::from(id)),
            classes: pending.classes,
            attrs: pending.attrs,
        }))
    }
}

impl<'a, 'o, I> Iterator for HeadingIds<'a, 'o, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.queue.pop_front() {
            return Some(event);
        }

        loop {
            match self.inner.next()? {
                Event::Start(Tag::Heading {
                    level,
                    id,
                    classes,
                    attrs,
                }) => {
                    if level == HeadingLevel::H1 && self.heading_index == 0 {
                        self.heading_index += 1;
                        return Some(Event::Start(Tag::Heading {
                            level,
                            id,
                            classes,
                            attrs,
                        }));
                    }

                    self.pending = Some(PendingHeading {
                        id,
                        classes,
                        attrs,
                        text: String::new(),
                        events: Vec::new(),
                    });
                    continue;
                }
                Event::End(TagEnd::Heading(level)) if self.pending.is_some() => {
                    return self.flush(level);
                }
                event => {
                    if let Some(pending) = &mut self.pending {
                        append_heading_text(&mut pending.text, &event);
                        pending.events.push(event);
                        continue;
                    }
                    return Some(event);
                }
            }
        }
    }
}

fn append_heading_text(text: &mut String, event: &Event<'_>) {
    match event {
        Event::Text(value) | Event::Code(value) => text.push_str(value),
        Event::SoftBreak | Event::HardBreak => text.push(' '),
        _ => {}
    }
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for ch in text.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || matches!(lower, '\u{4e00}'..='\u{9fa5}') {
            slug.push(lower);
        } else if ch.is_whitespace() || ch == '-' {
            slug.push('-');
        }
    }
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').to_owned()
}

/// Resolve a wikilink target to a slugified post URL under `/posts/`.
fn wikilink_destination(target: &str) -> String {
    let (path, fragment) = match target.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (target, None),
    };

    let path = path
        .split('/')
        .map(slugify)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");

    let mut url = format!("/posts/{path}");
    if !url.ends_with('/') {
        url.push('/');
    }

    if let Some(fragment) = fragment {
        let fragment = slugify(fragment);
        if !fragment.is_empty() {
            url.push('#');
            url.push_str(&fragment);
        }
    }

    url
}

fn unique_id(base: String, used: &mut HashSet<String>) -> String {
    let base = if base.is_empty() {
        "section".to_owned()
    } else {
        base
    };
    let mut id = base.clone();
    let mut counter = 1;
    while used.contains(&id) {
        id = format!("{base}-{counter}");
        counter += 1;
    }
    used.insert(id.clone());
    id
}

/// Replace math events with KaTeX HTML + MathML rendered by `katex-rs`.
///
/// The context (function/symbol tables) is built once via [`LazyLock`]; settings
/// are created once per renderer. No buffering is needed because each math
/// event maps to a single HTML event.
struct MathRenderer<I> {
    inner: I,
    inline_settings: Settings,
    display_settings: Settings,
}

impl<I> MathRenderer<I> {
    fn new(inner: I) -> Self {
        Self {
            inner,
            inline_settings: Settings::builder().throw_on_error(false).build(),
            display_settings: Settings::builder()
                .display_mode(true)
                .throw_on_error(false)
                .build(),
        }
    }
}

impl<'a, I> Iterator for MathRenderer<I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Event::InlineMath(tex) => Some(Event::Html(CowStr::from(render_math(
                &tex,
                &self.inline_settings,
            )))),
            Event::DisplayMath(tex) => Some(Event::Html(CowStr::from(render_math(
                &tex,
                &self.display_settings,
            )))),
            other => Some(other),
        }
    }
}

fn render_math(tex: &str, settings: &Settings) -> String {
    render_to_string(&KATEX_CONTEXT, tex, settings)
        .map(|html| canonicalize_katex_classes(&html))
        .unwrap_or_else(|_| format!("<code class=\"math-error\">{}</code>", escape_html(tex)))
}

/// `katex-rs` prefixes internal classes (`katex-base`, `katex-strut`, ...).
/// Official KaTeX stylesheets expect the unprefixed names, so map them back.
fn canonicalize_katex_classes(html: &str) -> String {
    const INTERNAL: [(&str, &str); 20] = [
        ("katex-accent", "accent"),
        ("katex-base", "base"),
        ("katex-fix", "fix"),
        ("katex-hdashline", "hdashline"),
        ("katex-hline", "hline"),
        ("katex-inner", "inner"),
        ("katex-newline", "newline"),
        ("katex-overlay", "overlay"),
        ("katex-overline", "overline"),
        ("katex-root", "root"),
        ("katex-rule", "rule"),
        ("katex-sizing", "sizing"),
        ("katex-smash", "smash"),
        ("katex-sout", "sout"),
        ("katex-stretchy", "stretchy"),
        ("katex-strut", "strut"),
        ("katex-tag", "tag"),
        ("katex-thinbox", "thinbox"),
        ("katex-underline", "underline"),
        ("katex-vbox", "vbox"),
    ];

    let mut html = html.to_owned();
    for (prefixed, canonical) in INTERNAL {
        html = html.replace(prefixed, canonical);
    }
    html
}

/// Make wikilink destinations slugified post URLs under `/posts/`.
///
/// `pulldown-cmark` emits the raw wikiname for `[[Page]]`, which would resolve
/// under the current post directory (`/posts/post/Page`). Turn it into the same
/// slug style used for headings/posts instead: `/posts/page/` or
/// `/posts/page/#section`.
struct WikiLinkRoots<I> {
    inner: I,
}

impl<I> WikiLinkRoots<I> {
    fn new(inner: I) -> Self {
        Self { inner }
    }
}

impl<'a, I> Iterator for WikiLinkRoots<I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|event| match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) if matches!(link_type, LinkType::WikiLink { .. }) => {
                let dest_url = CowStr::from(wikilink_destination(&dest_url));
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
            other => other,
        })
    }
}

/// Pass events through while recording YAML front matter.
struct MetadataCapture<'o, I> {
    inner: I,
    metadata: &'o mut Option<String>,
    in_metadata: bool,
}

impl<'o, I> MetadataCapture<'o, I> {
    fn new(inner: I, metadata: &'o mut Option<String>) -> Self {
        Self {
            inner,
            metadata,
            in_metadata: false,
        }
    }
}

impl<'o, 'a, I> Iterator for MetadataCapture<'o, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let event = self.inner.next()?;

        match &event {
            Event::Start(Tag::MetadataBlock(_)) => {
                self.in_metadata = true;
                *self.metadata = Some(String::new());
            }
            Event::Text(text) if self.in_metadata => {
                if let Some(metadata) = self.metadata.as_mut() {
                    metadata.push_str(text.as_ref());
                }
            }
            Event::End(TagEnd::MetadataBlock(_)) => self.in_metadata = false,
            _ => {}
        }

        Some(event)
    }
}

/// Replace plain text with `<abbr>` markup, skipping code/math/raw HTML.
struct AbbreviationExpander<'a, 'b, I> {
    inner: I,
    abbreviations: &'b [Abbreviation],
    pending: VecDeque<Event<'a>>,
    skip_depth: usize,
}

impl<'a, 'b, I> AbbreviationExpander<'a, 'b, I> {
    fn new(inner: I, abbreviations: &'b [Abbreviation]) -> Self {
        Self {
            inner,
            abbreviations,
            pending: VecDeque::new(),
            skip_depth: 0,
        }
    }
}

impl<'a, 'b, I> Iterator for AbbreviationExpander<'a, 'b, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }

        let event = self.inner.next()?;

        match event {
            Event::Start(tag @ (Tag::CodeBlock(_) | Tag::MetadataBlock(_))) => {
                self.skip_depth += 1;
                Some(Event::Start(tag))
            }
            Event::End(end @ (TagEnd::CodeBlock | TagEnd::MetadataBlock(_))) => {
                self.skip_depth = self.skip_depth.saturating_sub(1);
                Some(Event::End(end))
            }
            Event::Text(text) if self.skip_depth == 0 => {
                let mut events = expand_text(text, self.abbreviations).into_iter();
                let first = events.next().expect("expand_text always yields an event");
                self.pending.extend(events);
                Some(first)
            }
            other => Some(other),
        }
    }
}

/// Collect footnote definitions and emit them at the end.
///
/// `pulldown-cmark` yields definitions where they appear in the document. This
/// buffers them so a rendered page always ends with the notes, regardless of
/// where they were written. A separating rule is no longer emitted: the notes
/// are hidden sources for the popover shown by `app.js`.
struct FootnotesToEnd<'a, I> {
    inner: I,
    buffered: Vec<Event<'a>>,
    depth: usize,
    queue: VecDeque<Event<'a>>,
}

impl<'a, I> FootnotesToEnd<'a, I> {
    fn new(inner: I) -> Self {
        Self {
            inner,
            buffered: Vec::new(),
            depth: 0,
            queue: VecDeque::new(),
        }
    }
}

impl<'a, I> Iterator for FootnotesToEnd<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(event) = self.queue.pop_front() {
            return Some(event);
        }

        loop {
            match self.inner.next() {
                Some(event) => {
                    if self.depth > 0 || matches!(&event, Event::Start(Tag::FootnoteDefinition(_)))
                    {
                        match &event {
                            Event::Start(Tag::FootnoteDefinition(_)) => self.depth += 1,
                            Event::End(TagEnd::FootnoteDefinition) => {
                                self.depth = self.depth.saturating_sub(1)
                            }
                            _ => {}
                        }
                        self.buffered.push(event);
                        continue;
                    }
                    return Some(event);
                }
                None => {
                    if self.buffered.is_empty() {
                        return None;
                    }
                    self.queue.extend(self.buffered.drain(..));
                    return self.queue.pop_front();
                }
            }
        }
    }
}

/// Buffer each code block and replace it with `syntect`-highlighted HTML.
///
/// Only the current code block is buffered; the rest of the event stream is
/// processed lazily, following the approach in the referenced syntect guide.
struct CodeHighlighter<'t, I> {
    inner: I,
    /// `None` means emit classed spans and let a stylesheet supply the colors.
    theme: Option<&'t Theme>,
    code: String,
    language: String,
    in_code_block: bool,
}

impl<'t, I> CodeHighlighter<'t, I> {
    fn new(inner: I, theme: Option<&'t Theme>) -> Self {
        Self {
            inner,
            theme,
            code: String::new(),
            language: String::new(),
            in_code_block: false,
        }
    }
}

impl<'a, 't, I> Iterator for CodeHighlighter<'t, I>
where
    I: Iterator<Item = Event<'a>>,
{
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.in_code_block {
                match self.inner.next()? {
                    Event::Text(text) => {
                        self.code.push_str(text.as_ref());
                        continue;
                    }
                    Event::End(TagEnd::CodeBlock) => {
                        self.in_code_block = false;
                        let html = match self.theme {
                            Some(theme) => wrap_copy_code_block(&highlight_code(
                                &self.code,
                                &self.language,
                                theme,
                            )),
                            None => wrap_classed_code_block(&highlight_code_classed(
                                &self.code,
                                &self.language,
                            )),
                        };
                        self.code.clear();
                        self.language.clear();
                        return Some(Event::Html(CowStr::from(html)));
                    }
                    // Code blocks normally contain only Text events. Ignore
                    // anything unexpected so the HTML event stream stays valid.
                    _ => continue,
                }
            }

            match self.inner.next()? {
                Event::Start(Tag::CodeBlock(kind)) => {
                    self.in_code_block = true;
                    self.code.clear();
                    self.language = match kind {
                        CodeBlockKind::Fenced(info) => info
                            .split_whitespace()
                            .next()
                            .unwrap_or_default()
                            .trim_start_matches('.')
                            .to_owned(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                other => return Some(other),
            }
        }
    }
}

/// Wrap a highlighted `<pre>` in Chiri's positioned container and add the
/// copy button and `copy-code-block` class used by `/app.js`.
fn wrap_copy_code_block(pre_html: &str) -> String {
    let pre_html = pre_html.strip_suffix('\n').unwrap_or(pre_html);
    let inner = if let Some(rest) = pre_html.strip_prefix("<pre ") {
        format!("<pre class=\"copy-code-block\" {rest}")
    } else if let Some(rest) = pre_html.strip_prefix("<pre>") {
        format!("<pre class=\"copy-code-block\">{rest}")
    } else {
        format!("<pre class=\"copy-code-block\">{pre_html}</pre>")
    };

    format!("<div class=\"copy-code-wrapper\">{COPY_CODE_BUTTON}{inner}</div>")
}

/// Wrap classed code spans in the container used by the copy button.
fn wrap_classed_code_block(inner: &str) -> String {
    let inner = inner.strip_suffix('\n').unwrap_or(inner);
    format!(
        "<div class=\"copy-code-wrapper\">{COPY_CODE_BUTTON}<pre class=\"copy-code-block hl-code\">{inner}</pre></div>"
    )
}

fn highlight_code(code: &str, language: &str, theme: &Theme) -> String {
    let syntax = syntax_for(language);

    highlighted_html_for_string(code, &SYNTAX_SET, syntax, theme)
        .unwrap_or_else(|_| fallback_code_html(code, language))
}

/// Highlight into `.hl-*` token classes instead of inline colors.
fn highlight_code_classed(code: &str, language: &str) -> String {
    let syntax = syntax_for(language);
    let mut generator =
        ClassedHTMLGenerator::new_with_class_style(syntax, &SYNTAX_SET, CODE_CLASS_STYLE);

    // The line-based API requires a trailing newline on every line.
    let normalized = (!code.ends_with('\n')).then(|| format!("{code}\n"));
    let source = normalized.as_deref().unwrap_or(code);

    for line in LinesWithEndings::from(source) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return fallback_code_html(code, language);
        }
    }
    generator.finalize()
}

fn syntax_for(language: &str) -> &'static SyntaxReference {
    if language.is_empty() {
        return SYNTAX_SET.find_syntax_plain_text();
    }

    SYNTAX_SET
        .find_syntax_by_token(language)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(language))
        .or_else(|| SYNTAX_SET.find_syntax_by_name(language))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
}

fn fallback_code_html(code: &str, language: &str) -> String {
    let mut html = String::from("<pre><code");

    if !language.is_empty() {
        html.push_str(" class=\"language-");
        html.push_str(&escape_html(language));
        html.push('"');
    }

    html.push('>');
    html.push_str(&escape_html(code));
    html.push_str("</code></pre>");
    html
}

/// Split one text event into text/HTML events with abbreviations wrapped.
fn expand_text<'a>(text: CowStr<'a>, abbreviations: &[Abbreviation]) -> Vec<Event<'a>> {
    if abbreviations.is_empty() {
        return vec![Event::Text(text)];
    }

    let source = text.as_ref();
    let mut events = Vec::new();
    let mut pos = 0;

    while pos < source.len() {
        let mut best: Option<(usize, usize, &Abbreviation)> = None;

        for abbreviation in abbreviations {
            let needle = abbreviation.short.as_str();
            if needle.is_empty() {
                continue;
            }

            let mut from = pos;
            while let Some(offset) = source[from..].find(needle) {
                let start = from + offset;
                let end = start + needle.len();

                if has_word_boundaries(source, start, end, needle) {
                    let better = match best {
                        None => true,
                        Some((best_start, best_end, _)) => {
                            start < best_start || (start == best_start && end > best_end)
                        }
                    };
                    if better {
                        best = Some((start, end, abbreviation));
                    }
                    break;
                }

                from = end;
            }
        }

        let Some((start, end, abbreviation)) = best else {
            break;
        };

        if start > pos {
            events.push(Event::Text(CowStr::from(source[pos..start].to_owned())));
        }

        // `Event::Html` is written raw in normal content but ignored inside
        // image alt text by pulldown-cmark's HTML writer, which is what we want.
        events.push(Event::Html(CowStr::from(format!(
            "<abbr title=\"{}\">",
            escape_html(&abbreviation.title)
        ))));
        events.push(Event::Text(CowStr::from(abbreviation.short.clone())));
        events.push(Event::Html(CowStr::from("</abbr>".to_owned())));

        pos = end;
    }

    if pos < source.len() {
        events.push(Event::Text(CowStr::from(source[pos..].to_owned())));
    }

    if events.is_empty() {
        events.push(Event::Text(text));
    }

    events
}

/// Avoid wrapping abbreviations inside larger words (e.g. `HTML5`).
fn has_word_boundaries(source: &str, start: usize, end: usize, needle: &str) -> bool {
    let left_is_word = needle.chars().next().is_some_and(char::is_alphanumeric);
    let right_is_word = needle.chars().last().is_some_and(char::is_alphanumeric);

    let left_ok = !left_is_word
        || source[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_alphanumeric());
    let right_ok = !right_is_word
        || source[end..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_alphanumeric());

    left_ok && right_ok
}

/// Strip abbreviation definitions from the Markdown and collect them.
///
/// Definitions use Markdown Extra syntax: `*[HTML]: HyperText Markup Language`.
/// Fenced code blocks are left alone.
fn extract_abbreviations(markdown: &str) -> (String, Vec<Abbreviation>) {
    let mut cleaned = String::with_capacity(markdown.len());
    let mut abbreviations = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    let mut in_metadata = false;
    let mut metadata_seen = false;

    for line in markdown.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);

        if in_metadata {
            cleaned.push_str(line);
            if matches!(body.trim(), "---" | "...") {
                in_metadata = false;
            }
            continue;
        }

        if !metadata_seen && fence.is_none() && body.trim() == "---" {
            metadata_seen = true;
            in_metadata = true;
            cleaned.push_str(line);
            continue;
        }

        if let Some((marker, len)) = fence_marker(body) {
            match fence {
                None => fence = Some((marker, len)),
                Some((open_marker, open_len)) if marker == open_marker && len >= open_len => {
                    fence = None;
                }
                Some(_) => {}
            }
            cleaned.push_str(line);
            continue;
        }

        if fence.is_none()
            && let Some(abbreviation) = parse_abbreviation_definition(body)
        {
            abbreviations.push(abbreviation);
            continue;
        }

        cleaned.push_str(line);
    }

    // Longest first so `C++` wins over `C` when both are defined.
    abbreviations.sort_by(|a, b| {
        b.short
            .len()
            .cmp(&a.short.len())
            .then_with(|| a.short.cmp(&b.short))
    });

    (cleaned, abbreviations)
}

fn fence_marker(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }

    let len = trimmed.chars().take_while(|&ch| ch == marker).count();
    (len >= 3).then_some((marker, len))
}

fn parse_abbreviation_definition(line: &str) -> Option<Abbreviation> {
    let indent = line.len() - line.trim_start().len();
    if indent > 3 {
        return None;
    }

    let rest = line[indent..].strip_prefix("*[")?;
    let (short, title) = rest.split_once("]:")?;
    let short = short.trim();
    let title = title.trim();

    if short.is_empty() || title.is_empty() {
        return None;
    }

    Some(Abbreviation {
        short: short.to_owned(),
        title: title.to_owned(),
    })
}

pub(crate) fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }

    escaped
}
