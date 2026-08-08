//! Markdown formatter.

use crate::{ManMap, OptionMeta, OptionMetaMap};
use anyhow::Error;

pub(crate) struct MdFormatter {
    man_map: ManMap,
    option_meta: OptionMetaMap,
}

impl MdFormatter {
    pub(crate) fn new(man_map: ManMap, option_meta: OptionMetaMap) -> MdFormatter {
        MdFormatter {
            man_map,
            option_meta,
        }
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
        command_path: &str,
        arg_id: Option<&str>,
    ) -> Result<String, Error> {
        let rendered_body = render_option_body(block.trim());
        let rendered_body = merge_generated_option_meta(
            rendered_body,
            option_meta_for(params, command_path, arg_id, self),
        );
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

fn option_meta_for<'a>(
    params: &[&str],
    command_path: &str,
    arg_id: Option<&str>,
    formatter: &'a MdFormatter,
) -> Vec<&'a OptionMeta> {
    if let Some(arg_id) = arg_id
        && let Some(meta) = formatter
            .option_meta
            .get(&(command_path.to_owned(), arg_id.to_owned()))
    {
        return meta.iter().collect();
    }

    for key in option_lookup_keys(params) {
        if let Some(meta) = formatter.option_meta.get(&(command_path.to_owned(), key)) {
            return meta.iter().collect();
        }
    }

    Vec::new()
}

fn option_lookup_keys(params: &[&str]) -> Vec<String> {
    let mut keys = Vec::new();
    for param in params {
        keys.extend(extract_code_tokens(param).into_iter().filter(|token| {
            token.starts_with("--") || (token.starts_with('-') && token.len() == 2)
        }));
    }

    if keys.is_empty() {
        keys.extend(params.iter().map(|param| normalize_option_key(param)));
    }

    keys
}

fn extract_code_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = value;

    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        tokens.push(rest[..end].trim().to_owned());
        rest = &rest[end + 1..];
    }

    tokens
}

fn normalize_option_key(value: &str) -> String {
    let mut value = value.trim();
    value = value.trim_matches('`');
    value = value.trim_matches('_');
    value = value.trim_matches('[');
    value = value.trim_matches(']');
    value = value.trim_end_matches("...");
    value = value.trim_matches('_');
    value
        .trim()
        .trim_matches('<')
        .trim_matches('>')
        .trim_matches('_')
        .replace('_', "-")
        .to_ascii_lowercase()
}

fn merge_generated_option_meta(rendered_body: String, generated_meta: Vec<&OptionMeta>) -> String {
    let mut body = rendered_body;

    for meta in generated_meta {
        let rendered_meta = render_option_meta(&meta.label, &meta.value);
        if let Some(replaced) = replace_option_meta(&body, &meta.label, &rendered_meta) {
            body = replaced;
        } else {
            if !body.trim().is_empty() {
                body.push_str("\n\n");
            }
            body.push_str(&rendered_meta);
        }
    }

    body
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
        return render_option_meta(label, value);
    }

    trimmed.to_string()
}

fn render_option_meta(label: &str, value: &str) -> String {
    format!("<CommandOptionMeta label={label:?}>\n\n{value}\n\n</CommandOptionMeta>")
}

fn replace_option_meta(block: &str, label: &str, replacement: &str) -> Option<String> {
    let marker = format!("<CommandOptionMeta label={label:?}>");
    let start = block.find(&marker)?;
    let search_from = start + marker.len();
    let close = "</CommandOptionMeta>";
    let end = search_from + block[search_from..].find(close)? + close.len();

    let mut replaced = String::with_capacity(block.len() + replacement.len());
    replaced.push_str(&block[..start]);
    replaced.push_str(replacement);
    replaced.push_str(&block[end..]);
    Some(replaced)
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

    if let Some(value) = paragraph.strip_prefix("This conflicts with ") {
        return Some(("Conflicts with", value));
    }

    if let Some(value) = paragraph.strip_prefix("Requires ") {
        return Some(("Requires", value));
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
    use crate::format::Formatter;
    use crate::{ManMap, OptionMeta, OptionMetaMap};

    #[test]
    fn renders_options_as_mdx_components() {
        let formatter = MdFormatter::new(ManMap::new(), OptionMetaMap::new());
        let rendered = format!(
            "{}{}{}",
            formatter.render_options_start(),
            formatter
                .render_option(
                    &["`--flag` _value_"],
                    "Description.\n\nMore details.",
                    "acton",
                    "acton test",
                    None,
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
        let formatter = MdFormatter::new(ManMap::new(), OptionMetaMap::new());
        let rendered = formatter
            .render_option(
                &["`--color` _when_"],
                "Control when to use colored output.\n\nPossible values: `auto`, `always`, `never`\n\nDefaults to `auto`.\n\nRequires `--config`.\n\nThis conflicts with `--no-color`.\n\nMay be passed multiple times.",
                "acton",
                "acton test",
                None,
            )
            .expect("option should render");

        assert!(rendered.contains("Control when to use colored output."));
        assert!(rendered.contains("<CommandOptionMeta label=\"Possible values\">"));
        assert!(rendered.contains("`auto`, `always`, `never`"));
        assert!(rendered.contains("<CommandOptionMeta label=\"Default\">"));
        assert!(rendered.contains("`auto`."));
        assert!(rendered.contains("<CommandOptionMeta label=\"Requires\">"));
        assert!(rendered.contains("`--config`."));
        assert!(rendered.contains("<CommandOptionMeta label=\"Conflicts with\">"));
        assert!(rendered.contains("`--no-color`."));
        assert!(rendered.contains("<CommandOptionMeta label=\"Repeatable\">"));
    }

    #[test]
    fn renders_generated_option_metadata_from_command_path_and_arg_id() {
        let mut option_meta = OptionMetaMap::new();
        option_meta.insert(
            ("acton test".to_owned(), "baseline_snapshot".to_owned()),
            vec![OptionMeta {
                label: "Conflicts with".to_owned(),
                value: "`--snapshot`".to_owned(),
            }],
        );
        let formatter = MdFormatter::new(ManMap::new(), option_meta);

        let rendered = formatter
            .render_option(
                &["`--baseline-snapshot` _path_"],
                "Compare with a gas snapshot.",
                "acton-test",
                "acton test",
                Some("baseline_snapshot"),
            )
            .expect("option should render");

        assert!(rendered.contains("<CommandOptionMeta label=\"Conflicts with\">"));
        assert!(rendered.contains("`--snapshot`"));
    }

    #[test]
    fn generated_option_metadata_replaces_manual_metadata_for_same_label() {
        let mut option_meta = OptionMetaMap::new();
        option_meta.insert(
            ("acton wrapper".to_owned(), "--ts".to_owned()),
            vec![OptionMeta {
                label: "Conflicts with".to_owned(),
                value: "`--test`, `--test-output`".to_owned(),
            }],
        );
        let formatter = MdFormatter::new(ManMap::new(), option_meta);

        let rendered = formatter
            .render_option(
                &["`--ts`"],
                "Generate TypeScript.\n\nConflicts with test stub generation.",
                "acton-wrapper",
                "acton wrapper",
                None,
            )
            .expect("option should render");

        assert!(rendered.contains("`--test`, `--test-output`"));
        assert!(!rendered.contains("test stub generation."));
    }
}
