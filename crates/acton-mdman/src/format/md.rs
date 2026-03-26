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
        ""
    }

    fn render_options_end(&self) -> &'static str {
        ""
    }

    fn render_option(
        &self,
        params: &[&str],
        block: &str,
        _man_name: &str,
    ) -> Result<String, Error> {
        Ok(format!(
            "#### {}\n\n{}\n\n",
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
