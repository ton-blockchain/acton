//! mdman markdown to man converter.
//!
//! > This crate is maintained by the Cargo team, primarily for use by Cargo
//! > and not intended for external use (except as a transitive dependency). This
//! > crate may make major changes to its APIs or be deprecated without warning.
// Vendored from Cargo's mdman. Keep upstream diffs minimal and exempt this
// crate from the workspace-wide clippy policy.
#![allow(
    clippy::all,
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    clippy::restriction
)]

use anyhow::{Context, Error, bail};
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead};
use std::ops::Range;
use std::path::Path;
use url::Url;

mod format;
mod hbs;
mod util;

use format::Formatter;

/// Mapping of `(name, section)` of a man page to a URL.
pub type ManMap = HashMap<(String, u8), String>;

/// Structured metadata to render under an option in Markdown output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionMeta {
    pub label: String,
    pub value: String,
}

/// Mapping of `(man_name, option_key)` to structured option metadata.
pub type OptionMetaMap = HashMap<(String, String), Vec<OptionMeta>>;

/// A man section.
pub type Section = u8;

/// The output formats supported by mdman.
#[derive(Copy, Clone)]
pub enum Format {
    Man,
    Md,
    Text,
}

impl Format {
    /// The filename extension for the format.
    pub fn extension(&self, section: Section) -> String {
        match self {
            Format::Man => section.to_string(),
            Format::Md => "md".to_string(),
            Format::Text => "txt".to_string(),
        }
    }
}

/// Converts the handlebars markdown file at the given path into the given
/// format, returning the translated result.
pub fn convert(
    file: &Path,
    format: Format,
    url: Option<Url>,
    man_map: ManMap,
) -> Result<String, Error> {
    convert_with_option_meta(file, format, url, man_map, OptionMetaMap::new())
}

/// Converts the handlebars markdown file and enriches Markdown option blocks
/// with structured metadata.
pub fn convert_with_option_meta(
    file: &Path,
    format: Format,
    url: Option<Url>,
    man_map: ManMap,
    option_meta: OptionMetaMap,
) -> Result<String, Error> {
    let formatter: Box<dyn Formatter + Send + Sync> = match format {
        Format::Man => Box::new(format::man::ManFormatter::new(url)),
        Format::Md => Box::new(format::md::MdFormatter::new(man_map, option_meta)),
        Format::Text => Box::new(format::text::TextFormatter::new(url)),
    };
    let expanded = hbs::expand(file, &*formatter)?;
    // pulldown-cmark can behave a little differently with Windows newlines,
    // just normalize it.
    let expanded = expanded.replace("\r\n", "\n");
    formatter.render(&expanded)
}

/// Pulldown-cmark iterator yielding an `(event, range)` tuple.
type EventIter<'a> = Box<dyn Iterator<Item = (Event<'a>, Range<usize>)> + 'a>;

/// Creates a new markdown parser with the given input.
pub(crate) fn md_parser(input: &str, url: Option<Url>) -> EventIter<'_> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    let parser = Parser::new_ext(input, options);
    let parser = parser.into_offset_iter();
    // Translate all links to include the base url.
    let parser = parser.map(move |(event, range)| match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) if !matches!(link_type, LinkType::Email) => (
            Event::Start(Tag::Link {
                link_type,
                dest_url: join_url(url.as_ref(), dest_url),
                title,
                id,
            }),
            range,
        ),
        Event::End(TagEnd::Link) => (Event::End(TagEnd::Link), range),
        _ => (event, range),
    });
    Box::new(parser)
}

fn join_url<'a>(base: Option<&Url>, dest: CowStr<'a>) -> CowStr<'a> {
    match base {
        Some(base_url) => {
            // Absolute URL or page-relative anchor doesn't need to be translated.
            if dest.contains(':') || dest.starts_with('#') {
                dest
            } else {
                let joined = base_url.join(&dest).unwrap_or_else(|e| {
                    panic!("failed to join URL `{}` to `{}`: {}", dest, base_url, e)
                });
                String::from(joined).into()
            }
        }
        None => dest,
    }
}

pub fn extract_section(file: &Path) -> Result<Section, Error> {
    let f = fs::File::open(file).with_context(|| format!("could not open `{}`", file.display()))?;
    let mut f = io::BufReader::new(f);
    let mut line = String::new();
    f.read_line(&mut line)?;
    if !line.starts_with("# ") {
        bail!("expected input file to start with # header");
    }
    let (_name, section) = util::parse_name_and_section(&line[2..].trim()).with_context(|| {
        format!(
            "expected input file to have header with the format `# command-name(1)`, found: `{}`",
            line
        )
    })?;
    Ok(section)
}

#[cfg(test)]
mod tests {
    use super::{Format, ManMap, OptionMeta, OptionMetaMap, convert_with_option_meta};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn convert_with_option_meta_enriches_markdown_options() {
        let test_dir = temp_test_dir("convert_with_option_meta_enriches_markdown_options");
        fs::create_dir_all(test_dir.join("includes")).expect("must create includes dir");
        let source_path = test_dir.join("acton-test.md");
        fs::write(
            &source_path,
            r#"# acton-test(1)

## Options

{{#options command="acton test"}}

{{#option "`--baseline-snapshot` _path_" id="baseline_snapshot"}}
Compare gas usage against a snapshot.
{{/option}}

{{/options}}
"#,
        )
        .expect("must write source");

        let mut option_meta = OptionMetaMap::new();
        option_meta.insert(
            ("acton test".to_owned(), "baseline_snapshot".to_owned()),
            vec![OptionMeta {
                label: "Requires".to_owned(),
                value: "`--snapshot`".to_owned(),
            }],
        );

        let rendered =
            convert_with_option_meta(&source_path, Format::Md, None, ManMap::new(), option_meta)
                .expect("must render markdown");

        assert!(rendered.contains("<CommandOptionMeta label=\"Requires\">"));
        assert!(rendered.contains("`--snapshot`"));

        let _ = fs::remove_dir_all(test_dir);
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        PathBuf::from("/tmp").join(format!("acton-mdman-{name}-{}-{nanos}", std::process::id()))
    }
}
