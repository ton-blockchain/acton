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
        Ok(format!(
            "<CommandOption>\n\
<CommandOptionTitle>\n\n\
{}\n\n\
</CommandOptionTitle>\n\n\
{}\n\n\
</CommandOption>\n\n",
            params.join(", "),
            block.trim()
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
}
