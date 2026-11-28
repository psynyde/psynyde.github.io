//! A tiny, dependency-free HTML template renderer.
//!
//! Templates use two placeholders:
//!
//! - `{{ key }}` inserts HTML-escaped text.
//! - `{{{ key }}}` inserts a raw value (already-rendered HTML fragments).
//!
//! There are no loops or conditionals on purpose. Build optional fragments in
//! Rust and pass them in as raw values. Unknown keys are an error so typos do
//! not silently ship broken pages.

use std::collections::HashMap;

/// Fill `template` with `vars`.
///
/// # Errors
///
/// Returns the first missing key name if a placeholder has no value.
pub fn render(template: &str, vars: &HashMap<&str, String>) -> Result<String, String> {
    let mut output = String::with_capacity(template.len());
    let mut pos = 0;

    while pos < template.len() {
        if template[pos..].starts_with("{{{")
            && let Some(end) = template[pos + 3..].find("}}}")
        {
            let key = template[pos + 3..pos + 3 + end].trim();
            let value = vars
                .get(key)
                .ok_or_else(|| format!("missing template key: {key}"))?;
            output.push_str(value);
            pos += 3 + end + 3;
            continue;
        }

        if template[pos..].starts_with("{{")
            && let Some(end) = template[pos + 2..].find("}}")
        {
            let key = template[pos + 2..pos + 2 + end].trim();
            let value = vars
                .get(key)
                .ok_or_else(|| format!("missing template key: {key}"))?;
            output.push_str(&super::escape_html(value));
            pos += 2 + end + 2;
            continue;
        }

        let ch = template[pos..]
            .chars()
            .next()
            .expect("position is inside the template");
        output.push(ch);
        pos += ch.len_utf8();
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::render;
    use std::collections::HashMap;

    #[test]
    fn escapes_text_and_keeps_raw_html() {
        let vars = HashMap::from([
            ("title", "<Hello & goodbye>".to_owned()),
            ("body", "<p>raw</p>".to_owned()),
        ]);
        let html = render("{{ title }}{{{ body }}}", &vars).unwrap();

        assert_eq!(html, "&lt;Hello &amp; goodbye&gt;<p>raw</p>");
    }

    #[test]
    fn reports_missing_keys() {
        let vars = HashMap::from([("known", "value".to_owned())]);
        let error = render("{{ missing }}", &vars).unwrap_err();
        assert_eq!(error, "missing template key: missing");
    }

    #[test]
    fn handles_unicode_outside_placeholders() {
        let vars = HashMap::from([("name", "世界".to_owned())]);
        assert_eq!(render("🌸 {{ name }}", &vars).unwrap(), "🌸 世界");
    }
}
