//! Markdown formatter.

use crate::ManMap;
use anyhow::Error;

pub(crate) struct MdFormatter {
    man_map: ManMap,
}

impl MdFormatter {
    pub(crate) fn new(man_map: ManMap) -> MdFormatter {
        MdFormatter { man_map }
    }
}

impl super::Formatter for MdFormatter {
    fn render(&self, input: &str) -> Result<String, Error> {
        Ok(input.replace("\r\n", "\n"))
    }

    fn render_options_start(&self) -> &'static str {
        "<CommandOptions>\n\n"
    }

    fn render_options_end(&self) -> &'static str {
        "</CommandOptions>\n"
    }

    fn render_option(
        &self,
        params: &[&str],
        block: &str,
        _man_name: &str,
    ) -> Result<String, Error> {
        let rendered_body = render_option_body(block.trim());
        Ok(format!(
            "<CommandOption>\n\
<CommandOptionTitle>\n\n\
{}\n\n\
</CommandOptionTitle>\n\n\
{}\n\n\
</CommandOption>\n\n",
            params.join(", "),
            rendered_body
        ))
    }

    fn linkify_man_to_md(&self, name: &str, section: u8) -> Result<String, Error> {
        let s = match self.man_map.get(&(name.to_string(), section)) {
            Some(link) => format!("[{}({})]({})", name, section, link),
            None => format!("[{}({})]({}.html)", name, section, name),
        };
        Ok(s)
    }
}

fn render_option_body(block: &str) -> String {
    split_paragraphs(block)
        .into_iter()
        .map(|paragraph| render_option_paragraph(&paragraph))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn split_paragraphs(block: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();

    for line in block.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
            continue;
        }

        current.push(line.to_string());
    }

    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }

    paragraphs
}

fn render_option_paragraph(paragraph: &str) -> String {
    let trimmed = paragraph.trim();

    if let Some((label, value)) = classify_option_meta(trimmed) {
        return format!(
            "<CommandOptionMeta label={:?}>\n\n{}\n\n</CommandOptionMeta>",
            label, value
        );
    }

    trimmed.to_string()
}

fn classify_option_meta(paragraph: &str) -> Option<(&'static str, &str)> {
    if let Some(value) = paragraph.strip_prefix("Possible values: ") {
        return Some(("Possible values", value));
    }

    if let Some(value) = paragraph.strip_prefix("Defaults to ") {
        return Some(("Default", value));
    }

    if let Some(value) = paragraph.strip_prefix("Currently defaults to ") {
        return Some(("Current default", value));
    }

    if let Some(value) = paragraph.strip_prefix("Valid range: ") {
        return Some(("Valid range", value));
    }

    if let Some(value) = paragraph.strip_prefix("Conflicts with ") {
        return Some(("Conflicts with", value));
    }

    if let Some(value) = paragraph.strip_prefix("Ignored with ") {
        return Some(("Ignored with", value));
    }

    if let Some(value) = paragraph.strip_prefix("Also read from ") {
        return Some(("Environment", value));
    }

    if let Some(value) = paragraph.strip_prefix("If omitted, ") {
        return Some(("If omitted", value));
    }

    if paragraph.starts_with("May be passed multiple times") {
        return Some(("Repeatable", paragraph));
    }

    if paragraph.starts_with("Accepted as a global Acton option") {
        return Some(("Pass-through", paragraph));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::MdFormatter;
    use crate::ManMap;
    use crate::format::Formatter;

    #[test]
    fn renders_options_as_mdx_components() {
        let formatter = MdFormatter::new(ManMap::new());
        let rendered = format!(
            "{}{}{}",
            formatter.render_options_start(),
            formatter
                .render_option(
                    &["`--flag` _value_"],
                    "Description.\n\nMore details.",
                    "acton"
                )
                .expect("option should render"),
            formatter.render_options_end()
        );

        assert!(rendered.starts_with("<CommandOptions>\n\n"));
        assert!(
            rendered.contains("<CommandOptionTitle>\n\n`--flag` _value_\n\n</CommandOptionTitle>")
        );
        assert!(rendered.contains("Description.\n\nMore details."));
        assert!(rendered.ends_with("</CommandOptions>\n"));
    }

    #[test]
    fn renders_known_option_meta_paragraphs_as_structured_components() {
        let formatter = MdFormatter::new(ManMap::new());
        let rendered = formatter
            .render_option(
                &["`--color` _when_"],
                "Control when to use colored output.\n\nPossible values: `auto`, `always`, `never`\n\nDefaults to `auto`.\n\nMay be passed multiple times.",
                "acton",
            )
            .expect("option should render");

        assert!(rendered.contains("Control when to use colored output."));
        assert!(rendered.contains("<CommandOptionMeta label=\"Possible values\">"));
        assert!(rendered.contains("`auto`, `always`, `never`"));
        assert!(rendered.contains("<CommandOptionMeta label=\"Default\">"));
        assert!(rendered.contains("`auto`."));
        assert!(rendered.contains("<CommandOptionMeta label=\"Repeatable\">"));
    }
}
