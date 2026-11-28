//! Minimal file-based static site generator.
//!
//! Layout:
//!
//! - `content/about.md` — optional index-page intro
//! - `content/posts/**/*.md` — posts; each becomes `dist/posts/<slug>/index.html`
//! - `templates/*.html` — plain HTML templates with `{{ key }}` placeholders
//! - `static/**` — copied verbatim into `dist/`
//!
//! Front matter is the same YAML-style block already captured by the Markdown
//! renderer. Flat `key: value` pairs from Chiri-style posts are used for the
//! page title, date, description, and Open Graph image:
//!
//! ```markdown
//! ---
//! title: A post
//! pubDate: 2025-06-28
//! description: A short summary.
//! image: /images/cover.png
//! ---
//! ```
//!
//! No YAML dependency is needed; nested values are ignored.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    DEFAULT_THEME_FILE, TocItem, code_theme_css, escape_html, render_classed, template, theme_for,
};

/// Number of posts shown on the home page.
const HOME_POST_LIMIT: usize = 4;

/// Site-wide values used by the HTML templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteConfig {
    pub title: String,
    pub author: String,
    pub description: String,
    pub language: String,
    pub content_width: String,
    pub website: String,
    /// Code theme used when the site is in dark mode.
    pub theme: String,
    /// Code theme used when the site is in light mode; empty means `theme`.
    pub theme_light: String,
    pub copy_code: bool,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Chiri-Rust".to_owned(),
            author: "Psy".to_owned(),
            description: "A minimal static blog generated in Rust.".to_owned(),
            language: "en-US".to_owned(),
            content_width: "35rem".to_owned(),
            website: "https://example.com".to_owned(),
            theme: DEFAULT_THEME_FILE.to_owned(),
            theme_light: String::new(),
            copy_code: true,
        }
    }
}

impl SiteConfig {
    /// Load flat `key = value` settings from a small `site.toml`-style file.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or contains an unknown
    /// key or malformed line.
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let source = fs::read_to_string(path)?;
        let mut config = Self::default();

        for (index, line) in source.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(invalid_data(path, format!("line {} has no '='", index + 1)));
            };

            let key = key.trim();
            let value = unquote(value.trim()).to_owned();

            match key {
                "title" => config.title = value,
                "author" => config.author = value,
                "description" => config.description = value,
                "language" => config.language = value,
                "content_width" => config.content_width = value,
                "website" => config.website = value,
                "theme" => config.theme = value,
                "theme_light" => config.theme_light = value,
                "copy_code" => {
                    config.copy_code = match value.as_str() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(invalid_data(
                                path,
                                format!(
                                    "line {}: copy_code must be true or false, got {other}",
                                    index + 1
                                ),
                            ));
                        }
                    };
                }
                other => {
                    return Err(invalid_data(path, format!("unknown setting: {other}")));
                }
            }
        }

        Ok(config)
    }
}

/// Input and output paths for a build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOptions {
    pub content_dir: PathBuf,
    pub templates_dir: PathBuf,
    pub static_dir: PathBuf,
    pub output_dir: PathBuf,
    pub config: SiteConfig,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            content_dir: PathBuf::from("content"),
            templates_dir: PathBuf::from("templates"),
            static_dir: PathBuf::from("static"),
            output_dir: PathBuf::from("dist"),
            config: SiteConfig::default(),
        }
    }
}

/// What the build produced, useful for a short CLI summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildSummary {
    pub posts: usize,
    pub has_about: bool,
}

/// Build the site into `options.output_dir`.
///
/// Existing files are overwritten; unrelated files in the output directory are
/// left alone.
///
/// # Errors
///
/// Returns an error for missing content, unreadable templates, bad template
/// placeholders, or failed file copies.
pub fn build(options: &BuildOptions) -> io::Result<BuildSummary> {
    if !options.content_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "content directory not found: {}",
                options.content_dir.display()
            ),
        ));
    }

    let dark_theme = theme_for(&options.config.theme)?;
    let light_name = if options.config.theme_light.is_empty() {
        &options.config.theme
    } else {
        &options.config.theme_light
    };
    let light_theme = theme_for(light_name)?;
    let code_css = format!(
        "{}\n{}",
        code_theme_css(&dark_theme, "html.dark")?,
        code_theme_css(&light_theme, "html.light")?
    );

    fs::create_dir_all(&options.output_dir)?;
    copy_dir_all(&options.static_dir, &options.output_dir)?;
    fs::write(options.output_dir.join("code-theme.css"), code_css)?;

    let templates = Templates::new(&options.templates_dir);
    let header = templates.render(
        "header",
        &HashMap::from([("site_title", options.config.title.clone())]),
    )?;
    let footer = templates.render(
        "footer",
        &HashMap::from([
            ("year", current_year().to_string()),
            ("author", options.config.author.clone()),
        ]),
    )?;

    let about = render_about(options, &templates)?;
    let posts = collect_posts(options, &templates, &footer)?;

    remove_legacy_post_pages(&options.output_dir, &posts)?;
    render_index(options, &templates, &header, &footer, &about, &posts)?;
    render_posts_page(options, &templates, &header, &footer, &posts)?;

    Ok(BuildSummary {
        posts: posts.len(),
        has_about: !about.is_empty(),
    })
}

struct Templates {
    dir: PathBuf,
}

impl Templates {
    fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
        }
    }

    fn render(&self, name: &str, vars: &HashMap<&str, String>) -> io::Result<String> {
        let path = self.dir.join(format!("{name}.html"));
        let source = fs::read_to_string(&path)?;
        template::render(&source, vars).map_err(|error| invalid_data(&path, error))
    }
}

#[derive(Debug, Clone)]
struct Post {
    slug: String,
    title: String,
    date: String,
    datetime: String,
}

fn render_about(options: &BuildOptions, templates: &Templates) -> io::Result<String> {
    let path = options.content_dir.join("about.md");
    if !path.is_file() {
        return Ok(String::new());
    }

    let markdown = fs::read_to_string(&path)?;
    let content = render_classed(&markdown).html;
    if content.trim().is_empty() {
        return Ok(String::new());
    }
    templates.render("about", &HashMap::from([("content", content)]))
}

/// Build Chiri's left-side table-of-contents rail from rendered headings.
fn render_toc(items: &[TocItem]) -> String {
    if items.is_empty() {
        return String::new();
    }

    let mut html = String::from(
        "<div class=\"toc-container\" id=\"toc\">\n\
         \x20 <nav class=\"toc-nav\">\n\
         \x20   <ul class=\"toc-list\" id=\"toc-list\">\n\
         \x20     <li class=\"toc-item toc-level-0\">\n\
         \x20       <a href=\"#\" class=\"toc-link toc-title\" aria-label=\"Back to top\" data-text=\"Back to top\"> Back to top </a>\n\
         \x20     </li>\n",
    );

    for item in items {
        let level = item.level.clamp(1, 3);
        let label = escape_html(&item.text);
        let id = escape_html(&item.id);
        html.push_str(&format!(
            "      <li class=\"toc-item toc-level-{level}\">\n\
             \x20       <a href=\"#{id}\" class=\"toc-link\" aria-label=\"{label}\" data-text=\"{label}\">{label}</a>\n\
             \x20     </li>\n"
        ));
    }

    html.push_str("    </ul>\n  </nav>\n</div>\n");
    html
}

fn collect_posts(
    options: &BuildOptions,
    templates: &Templates,
    footer: &str,
) -> io::Result<Vec<Post>> {
    let posts_dir = options.content_dir.join("posts");
    let mut files = markdown_files(&posts_dir)?;
    let mut posts = Vec::with_capacity(files.len());

    for path in files.drain(..) {
        let Some(slug) = slug_for(&path, &posts_dir)? else {
            continue;
        };

        let markdown = fs::read_to_string(&path)?;
        let rendered = render_classed(&markdown);
        let metadata = parse_metadata(rendered.metadata.as_deref().unwrap_or_default());

        if metadata
            .get("draft")
            .is_some_and(|draft| draft.eq_ignore_ascii_case("true"))
        {
            continue;
        }

        let title = metadata
            .get("title")
            .filter(|title| !title.is_empty())
            .cloned()
            .or_else(|| first_heading(&markdown))
            .unwrap_or_else(|| title_case(&slug));
        let datetime = metadata
            .get("pubdate")
            .or_else(|| metadata.get("date"))
            .cloned()
            .unwrap_or_default();
        let date = display_date(&datetime);
        let description = metadata
            .get("description")
            .filter(|description| !description.is_empty())
            .cloned()
            .unwrap_or_else(|| options.config.description.clone());
        let image = metadata
            .get("image")
            .filter(|image| !image.is_empty())
            .cloned()
            .unwrap_or_default();

        let toc = render_toc(&rendered.toc);
        let body = templates.render(
            "post",
            &HashMap::from([
                ("title", title.clone()),
                ("date", date.clone()),
                ("datetime", datetime.clone()),
                ("toc", toc),
                ("content", rendered.html),
            ]),
        )?;
        let page_title = format!("{title} · {}", options.config.title);
        let head_extra = head_tags(
            &options.config,
            &format!("/posts/{slug}/"),
            &page_title,
            &description,
            &image,
        );
        let page = wrap_page(
            templates,
            &options.config,
            &PageContext {
                title: &page_title,
                description: &description,
                head_extra: &head_extra,
                page_class: "post-container",
                header: "",
                footer,
                body: &body,
            },
        )?;

        let post_dir = options.output_dir.join("posts").join(&slug);
        fs::create_dir_all(&post_dir)?;
        fs::write(post_dir.join("index.html"), page)?;

        posts.push(Post {
            slug,
            title,
            date,
            datetime,
        });
    }

    posts.sort_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.slug.cmp(&b.slug))
    });

    Ok(posts)
}

fn render_index(
    options: &BuildOptions,
    templates: &Templates,
    header: &str,
    footer: &str,
    about: &str,
    posts: &[Post],
) -> io::Result<()> {
    let visible = &posts[..posts.len().min(HOME_POST_LIMIT)];
    let items = post_items(visible, templates)?;
    let more = templates.render("posts-more", &HashMap::<&str, String>::new())?;

    let body = templates.render(
        "index",
        &HashMap::from([
            ("about", about.to_owned()),
            ("posts", items),
            ("more", more),
        ]),
    )?;
    let head_extra = head_tags(
        &options.config,
        "/",
        &options.config.title,
        &options.config.description,
        "",
    );
    let page = wrap_page(
        templates,
        &options.config,
        &PageContext {
            title: &options.config.title,
            description: &options.config.description,
            head_extra: &head_extra,
            page_class: "page-content",
            header,
            footer,
            body: &body,
        },
    )?;

    fs::write(options.output_dir.join("index.html"), page)
}

fn render_posts_page(
    options: &BuildOptions,
    templates: &Templates,
    header: &str,
    footer: &str,
    posts: &[Post],
) -> io::Result<()> {
    let items = post_items(posts, templates)?;
    let body = templates.render("posts", &HashMap::from([("posts", items)]))?;
    let page_title = format!("Posts · {}", options.config.title);
    let head_extra = head_tags(
        &options.config,
        "/posts/",
        &page_title,
        &options.config.description,
        "",
    );
    let page = wrap_page(
        templates,
        &options.config,
        &PageContext {
            title: &page_title,
            description: &options.config.description,
            head_extra: &head_extra,
            page_class: "page-content",
            header,
            footer,
            body: &body,
        },
    )?;

    let posts_dir = options.output_dir.join("posts");
    fs::create_dir_all(&posts_dir)?;
    fs::write(posts_dir.join("index.html"), page)
}

fn post_items(posts: &[Post], templates: &Templates) -> io::Result<String> {
    let mut items = String::new();
    for post in posts {
        let item = templates.render(
            "post-item",
            &HashMap::from([
                ("slug", post.slug.clone()),
                ("title", post.title.clone()),
                ("date", post.date.clone()),
                ("datetime", post.datetime.clone()),
            ]),
        )?;
        items.push_str(&item);
    }
    Ok(items)
}

/// Remove pages generated by older builds at `/<slug>/`, leaving non-empty
/// directories (for example copied static assets) untouched.
fn remove_legacy_post_pages(output_dir: &Path, posts: &[Post]) -> io::Result<()> {
    for post in posts {
        let relative = Path::new(&post.slug);
        // A post whose slug already starts with `posts` now lives at the path
        // this cleanup would otherwise remove.
        if relative.starts_with("posts") {
            continue;
        }

        let old_dir = output_dir.join(relative);
        let old_index = old_dir.join("index.html");
        if old_index.is_file() {
            fs::remove_file(old_index)?;
        }
        remove_empty_parents(output_dir, &old_dir)?;
    }
    Ok(())
}

fn remove_empty_parents(root: &Path, start: &Path) -> io::Result<()> {
    let mut dir = start;
    while dir != root && dir.starts_with(root) {
        if !dir.is_dir() || fs::read_dir(dir)?.next().is_some() {
            break;
        }
        fs::remove_dir(dir)?;
        dir = dir.parent().unwrap_or(root);
    }
    Ok(())
}

struct PageContext<'a> {
    title: &'a str,
    description: &'a str,
    head_extra: &'a str,
    page_class: &'a str,
    header: &'a str,
    footer: &'a str,
    body: &'a str,
}

fn wrap_page(
    templates: &Templates,
    config: &SiteConfig,
    page: &PageContext<'_>,
) -> io::Result<String> {
    templates.render(
        "base",
        &HashMap::from([
            ("language", config.language.clone()),
            ("title", page.title.to_owned()),
            ("description", page.description.to_owned()),
            ("content_width", config.content_width.clone()),
            (
                "copy_code",
                if config.copy_code {
                    "enabled"
                } else {
                    "disabled"
                }
                .to_owned(),
            ),
            ("head_extra", page.head_extra.to_owned()),
            ("page_class", page.page_class.to_owned()),
            ("header", page.header.to_owned()),
            ("body", page.body.to_owned()),
            ("footer", page.footer.to_owned()),
        ]),
    )
}

/// Build Chiri-style metadata tags from post/site front matter.
fn head_tags(
    config: &SiteConfig,
    path: &str,
    title: &str,
    description: &str,
    image: &str,
) -> String {
    let base = config.website.trim_end_matches('/');
    let canonical = format!("{base}/{}", path.trim_start_matches('/'));
    let image_url = if image.is_empty() {
        format!("{base}/og.png")
    } else if image.starts_with("http://") || image.starts_with("https://") {
        image.to_owned()
    } else {
        format!("{base}/{}", image.trim_start_matches('/'))
    };

    let attr = |name: &str, value: &str| format!("{name}=\"{}\"", escape_html(value));
    let href = attr("href", &canonical);
    let content_title = attr("content", title);
    let content_url = attr("content", &canonical);
    let content_description = attr("content", description);
    let content_image = attr("content", &image_url);

    format!(
        r#"  <link rel="canonical" {href}>
  <meta name="title" {content_title}>
  <meta property="og:type" content="website">
  <meta property="og:url" {content_url}>
  <meta property="og:title" {content_title}>
  <meta property="og:description" {content_description}>
  <meta property="og:image" {content_image}>
  <meta name="twitter:card" content="summary_large_image">
  <meta name="twitter:url" {content_url}>
  <meta name="twitter:title" {content_title}>
  <meta name="twitter:description" {content_description}>
  <meta name="twitter:image" {content_image}>
"#
    )
}

fn markdown_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(markdown_files(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Return the URL-safe slug, or `None` for drafts (`_` prefix).
fn slug_for(path: &Path, posts_dir: &Path) -> io::Result<Option<String>> {
    let relative = path.strip_prefix(posts_dir).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("post is outside content directory: {error}"),
        )
    })?;

    let without_extension = relative.with_extension("");
    let mut parts = Vec::new();

    for component in without_extension.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.starts_with('_') {
            return Ok(None);
        }

        let sanitized: String = name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '-'
                }
            })
            .collect();

        if !sanitized.is_empty() {
            parts.push(sanitized);
        }
    }

    Ok((!parts.is_empty()).then(|| parts.join("/")))
}

fn copy_dir_all(from: &Path, to: &Path) -> io::Result<()> {
    if !from.exists() {
        return Ok(());
    }

    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    Ok(())
}

/// Read flat front-matter values. Nested YAML lists are ignored.
pub fn parse_metadata(raw: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            continue;
        }

        metadata.insert(key.to_ascii_lowercase(), unquote(value.trim()).to_owned());
    }

    metadata
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && bytes.first().is_some_and(|&b| b == b'\'' || b == b'"')
        && bytes.last() == bytes.first()
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn first_heading(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let line = line.trim_start();
        let level = line.chars().take_while(|&ch| ch == '#').count();
        if level == 0 || line.as_bytes().get(level) != Some(&b' ') {
            return None;
        }

        let heading = line[level..].trim().trim_end_matches('#').trim().to_owned();
        (!heading.is_empty()).then_some(heading)
    })
}

fn title_case(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_date(value: &str) -> String {
    let value = value.trim();
    if value.get(4..5) == Some("-") && value.get(7..8) == Some("-") {
        value[..10].replace('-', ".")
    } else {
        value.to_owned()
    }
}

fn current_year() -> i64 {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86_400;
    civil_from_days(days)
}

/// Howard Hinnant's days-since-epoch to civil-date conversion.
fn civil_from_days(days: i64) -> i64 {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    year + i64::from(month <= 2)
}

fn invalid_data(path: &Path, message: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {message}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::{BuildOptions, SiteConfig, parse_metadata};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir(test: &str) -> PathBuf {
        let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("blog-site-{test}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn parses_flat_front_matter() {
        let metadata = parse_metadata(
            "title: \"A post\"\npubDate: 2025-06-28\ntags:\n  - rust\n  - markdown\n",
        );
        assert_eq!(metadata.get("title").map(String::as_str), Some("A post"));
        assert_eq!(
            metadata.get("pubdate").map(String::as_str),
            Some("2025-06-28")
        );
    }

    #[test]
    fn reads_theme_and_website_from_config() {
        let root = temp_dir("config");
        let path = root.join("site.toml");
        fs::write(
            &path,
            "title = \"Notes\"\nwebsite = \"https://blog.test\"\ntheme = \"mono.tmTheme\"\ntheme_light = \"rose-pine-dawn.tmTheme\"\ncopy_code = false\n",
        )
        .unwrap();

        let config = SiteConfig::from_file(&path).unwrap();
        assert_eq!(config.title, "Notes");
        assert_eq!(config.website, "https://blog.test");
        assert_eq!(config.theme, "mono.tmTheme");
        assert_eq!(config.theme_light, "rose-pine-dawn.tmTheme");
        assert!(!config.copy_code);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn builds_index_and_post_pages() {
        let root = temp_dir("build");
        let content = root.join("content");
        let static_dir = root.join("static");
        let output = root.join("dist");
        fs::create_dir_all(content.join("posts")).unwrap();
        fs::create_dir_all(static_dir.join("fonts")).unwrap();
        fs::write(content.join("about.md"), "About **this** site.\n").unwrap();
        fs::write(
            content.join("posts").join("hello.md"),
            "---\n\
             title: Hello, world\n\
             pubDate: 2025-01-02\n\
             description: A short summary.\n\
             image: /images/hello.png\n\
             ---\n\n\
             ## A Section\n\n\
             $E = mc^2$\n\n\
             ```rust\nfn main() {}\n```\n",
        )
        .unwrap();
        fs::write(
            content.join("posts").join("_draft.md"),
            "---\ntitle: Draft\ndraft: true\n---\n\nHidden.\n",
        )
        .unwrap();
        fs::write(
            content.join("posts").join("hidden.md"),
            "---\ntitle: Hidden\ndraft: True\n---\n\nHidden.\n",
        )
        .unwrap();
        fs::write(static_dir.join("fonts").join("test.woff2"), "font").unwrap();
        let icons = static_dir.join("icons");
        fs::create_dir_all(&icons).unwrap();
        fs::write(icons.join("favicon.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>")
            .unwrap();

        let options = BuildOptions {
            content_dir: content,
            templates_dir: PathBuf::from("templates"),
            static_dir,
            output_dir: output.clone(),
            config: SiteConfig::default(),
        };
        let summary = super::build(&options).unwrap();

        assert_eq!(summary.posts, 1);
        assert!(summary.has_about);
        assert!(!output.join("draft").exists());
        assert!(!output.join("hidden").exists());

        let index = fs::read_to_string(output.join("index.html")).unwrap();
        assert!(index.contains("rel=\"icon\" type=\"image/svg+xml\" href=\"/icons/favicon.svg\""));
        assert!(index.contains("rel=\"manifest\" href=\"/icons/site.webmanifest\""));
        assert!(index.contains("id=\"theme-toggle\""));
        assert!(index.contains("data-copy-code=\"enabled\""));
        assert!(index.contains("hollow-circle"));
        assert!(index.contains("solid-circle"));
        assert!(index.contains("class=\"page-content\""));
        assert!(index.contains("class=\"post-item\""));
        assert!(index.contains("href=\"/posts/hello/\""));
        assert!(index.contains("Hello, world"));
        assert!(index.contains("<time datetime=\"2025-01-02\">2025.01.02</time>"));
        assert!(index.contains("href=\"/posts/\" class=\"posts-more\""));

        let posts_page = fs::read_to_string(output.join("posts").join("index.html")).unwrap();
        assert!(posts_page.contains("<ul class=\"posts-list\">"));
        assert!(posts_page.contains("class=\"post-item\""));
        assert!(posts_page.contains("href=\"/posts/hello/\""));
        assert!(!posts_page.contains("class=\"back-button\""));
        assert!(!posts_page.contains("class=\"posts-more\""));

        assert!(!output.join("hello").exists());
        let post =
            fs::read_to_string(output.join("posts").join("hello").join("index.html")).unwrap();
        assert!(post.contains("class=\"post-container\""));
        assert!(!post.contains("<header>"));
        assert!(post.contains("id=\"theme-toggle\""));
        assert!(post.contains("post-theme-toggle"));
        assert!(post.contains("class=\"toc-container\""));
        assert!(post.contains("data-text=\"A Section\""));
        assert!(!post.contains("toc-link\" title="));
        assert!(!post.contains("toc-title\" title="));
        assert!(post.contains("<h2 id=\"a-section\">A Section</h2>"));
        assert!(post.contains("katex"));
        assert!(post.contains("A short summary."));
        assert!(post.contains("og:image\" content=\"https://example.com/images/hello.png\""));
        assert!(post.contains("rel=\"canonical\" href=\"https://example.com/posts/hello/\""));
        assert!(post.contains("href=\"/\" class=\"back-button\""));
        assert!(post.contains("<pre class=\"copy-code-block hl-code\">"));
        assert!(post.contains("class=\"copy-code-wrapper\""));
        assert!(post.contains("class=\"copy-code-block hl-code\""));
        assert!(post.contains("class=\"copy-button\""));

        let code_css = fs::read_to_string(output.join("code-theme.css")).unwrap();
        assert!(code_css.contains("html.dark .hl-code"));
        assert!(code_css.contains("html.light .hl-code"));
        assert!(code_css.contains("background-color: #101010;"));

        assert!(output.join("fonts").join("test.woff2").is_file());
        assert!(output.join("icons").join("favicon.svg").is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn home_limits_posts_and_posts_page_lists_all() {
        let root = temp_dir("post-limit");
        let content = root.join("content");
        let static_dir = root.join("static");
        let output = root.join("dist");
        fs::create_dir_all(content.join("posts")).unwrap();
        fs::create_dir_all(&static_dir).unwrap();

        for number in 1..=5 {
            fs::write(
                content.join("posts").join(format!("post-{number}.md")),
                format!("---\ntitle: Post {number}\npubDate: 2025-01-0{number}\n---\n\nBody.\n"),
            )
            .unwrap();
        }

        let options = BuildOptions {
            content_dir: content,
            templates_dir: PathBuf::from("templates"),
            static_dir,
            output_dir: output.clone(),
            config: SiteConfig::default(),
        };
        let summary = super::build(&options).unwrap();
        assert_eq!(summary.posts, 5);

        let index = fs::read_to_string(output.join("index.html")).unwrap();
        let posts_page = fs::read_to_string(output.join("posts").join("index.html")).unwrap();

        assert_eq!(index.matches("class=\"post-item\"").count(), 4);
        assert_eq!(posts_page.matches("class=\"post-item\"").count(), 5);
        assert!(index.contains("href=\"/posts/post-5/\""));
        assert!(index.contains("href=\"/posts/post-2/\""));
        assert!(!index.contains("href=\"/posts/post-1/\""));
        assert!(posts_page.contains("href=\"/posts/post-5/\""));
        assert!(index.contains("class=\"posts-more\""));
        assert!(!posts_page.contains("class=\"posts-more\""));

        fs::remove_dir_all(root).unwrap();
    }
}
