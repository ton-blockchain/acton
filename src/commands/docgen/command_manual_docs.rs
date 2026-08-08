use acton_mdman::{Format, ManMap, OptionMeta, OptionMetaMap, convert_with_option_meta};
use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
use std::fs;
use std::path::Path;

use super::command_manuals::{COMMAND_MANUAL_SOURCE_DIR, COMMAND_MANUALS, CommandManualSpec};
use super::generated_notice_from_path;

const DOCS_SITE_PREFIX: &str = "https://ton-blockchain.github.io/acton/docs";
const DOCS_PATH_PREFIX: &str = "/docs";

pub(super) fn generate_command_manual_docs(
    command_docs_out_dir: &Path,
    cli_command: &Command,
) -> Result<()> {
    fs::create_dir_all(command_docs_out_dir)?;

    for spec in COMMAND_MANUALS {
        generate_single_command_manual_doc(spec, command_docs_out_dir, cli_command)?;
    }

    Ok(())
}

fn generate_single_command_manual_doc(
    spec: &CommandManualSpec,
    command_docs_out_dir: &Path,
    cli_command: &Command,
) -> Result<()> {
    let source_path = Path::new(COMMAND_MANUAL_SOURCE_DIR).join(spec.source_name);
    let option_meta = command_option_meta_map(spec, cli_command);
    let docs_body =
        convert_with_option_meta(&source_path, Format::Md, None, ManMap::new(), option_meta)
            .with_context(|| {
                format!("Failed to render markdown manual {}", source_path.display())
            })?;

    fs::write(
        command_docs_out_dir.join(format!("{}.mdx", spec.docs_slug)),
        render_docs_page(spec, &source_path, &docs_body),
    )?;

    Ok(())
}

fn render_docs_page(spec: &CommandManualSpec, source_path: &Path, body: &str) -> String {
    let body = rewrite_docs_links(body);
    let body = strip_leading_h1(&body);
    let body = strip_leading_section(body, "NAME");
    let generated_notice = generated_notice_from_path(source_path);
    format!(
        "---\ntitle: {:?}\ndescription: {:?}\n---\n\n{generated_notice}{body}",
        spec.docs_title, spec.docs_description
    )
}

fn rewrite_docs_links(body: &str) -> String {
    body.replace(DOCS_SITE_PREFIX, DOCS_PATH_PREFIX)
}

fn strip_leading_h1(body: &str) -> &str {
    let Some(rest) = body.strip_prefix("# ") else {
        return body;
    };
    let Some((_, rest)) = rest.split_once('\n') else {
        return body;
    };
    rest.trim_start_matches('\n')
}

fn strip_leading_section<'a>(body: &'a str, title: &str) -> &'a str {
    let Some(rest) = body.strip_prefix("## ") else {
        return body;
    };
    let Some((heading_title, rest)) = rest.split_once('\n') else {
        return body;
    };
    if !heading_title.eq_ignore_ascii_case(title) {
        return body;
    }

    match rest.find("\n## ") {
        Some(next_section_idx) => rest[next_section_idx + 1..].trim_start_matches('\n'),
        None => "",
    }
}

fn command_option_meta_map(spec: &CommandManualSpec, cli_command: &Command) -> OptionMetaMap {
    let Some(command) = cli_command
        .get_subcommands()
        .find(|command| command.get_name() == spec.command)
    else {
        return OptionMetaMap::new();
    };

    let mut option_meta = OptionMetaMap::new();
    let command_path = format!("{} {}", cli_command.get_name(), spec.command);
    collect_command_option_meta(command, &command_path, &mut option_meta);
    option_meta
}

fn collect_command_option_meta(
    command: &Command,
    command_path: &str,
    option_meta: &mut OptionMetaMap,
) {
    for arg in command.get_arguments().filter(|arg| !arg.is_hide_set()) {
        let meta = generated_option_meta(command, arg);
        if meta.is_empty() {
            continue;
        }

        for key in option_lookup_keys_for_arg(arg) {
            option_meta.insert((command_path.to_owned(), key), meta.clone());
        }
    }

    for subcommand in command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
    {
        let subcommand_path = format!("{command_path} {}", subcommand.get_name());
        collect_command_option_meta(subcommand, &subcommand_path, option_meta);
    }
}

fn option_lookup_keys_for_arg(arg: &Arg) -> Vec<String> {
    let mut keys = vec![arg.get_id().as_str().to_owned()];

    if let Some(long) = arg.get_long() {
        keys.push(format!("--{long}"));
    }
    if let Some(aliases) = arg.get_all_aliases() {
        keys.extend(aliases.into_iter().map(|alias| format!("--{alias}")));
    }

    if let Some(short) = arg.get_short() {
        keys.push(format!("-{short}"));
    }
    if let Some(short_aliases) = arg.get_all_short_aliases() {
        keys.extend(short_aliases.into_iter().map(|alias| format!("-{alias}")));
    }

    if arg.is_positional() {
        keys.push(normalize_name(arg.get_id().as_str()));
        if let Some(value_names) = arg.get_value_names() {
            keys.extend(
                value_names
                    .iter()
                    .map(|value_name| normalize_name(value_name.as_str())),
            );
        }
    }

    keys.sort();
    keys.dedup();
    keys
}

fn normalize_name(value: &str) -> String {
    value
        .trim()
        .trim_matches('<')
        .trim_matches('>')
        .trim_matches('_')
        .replace('_', "-")
        .to_ascii_lowercase()
}

fn generated_option_meta(command: &Command, arg: &Arg) -> Vec<OptionMeta> {
    let mut metas = Vec::new();

    let possible_values = generated_possible_values(arg);
    if !possible_values.is_empty() {
        metas.push(OptionMeta {
            label: "Possible values".to_owned(),
            value: possible_values,
        });
    }

    let default_values = generated_default_values(arg);
    if !default_values.is_empty() {
        metas.push(OptionMeta {
            label: "Default".to_owned(),
            value: default_values,
        });
    }

    let conflicts = generated_conflicts(command, arg);
    if !conflicts.is_empty() {
        metas.push(OptionMeta {
            label: "Conflicts with".to_owned(),
            value: conflicts,
        });
    }

    if matches!(arg.get_action(), ArgAction::Append | ArgAction::Count) {
        metas.push(OptionMeta {
            label: "Repeatable".to_owned(),
            value: "May be passed multiple times.".to_owned(),
        });
    }

    metas
}

fn generated_possible_values(arg: &Arg) -> String {
    if arg.is_hide_possible_values_set() {
        return String::new();
    }

    let values = arg
        .get_possible_values()
        .into_iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_owned())
        .collect::<Vec<_>>();

    render_code_list(&values)
}

fn generated_default_values(arg: &Arg) -> String {
    if arg.is_hide_default_value_set() {
        return String::new();
    }

    let values = arg
        .get_default_values()
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    if values.is_empty()
        || (values.len() == 1
            && values[0] == "false"
            && matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse))
    {
        return String::new();
    }

    render_code_list(&values)
}

fn generated_conflicts(command: &Command, arg: &Arg) -> String {
    let mut conflicts = Vec::new();

    for conflict in command.get_arg_conflicts_with(arg) {
        push_conflict_reference(&mut conflicts, conflict);
    }

    for other in command.get_arguments() {
        if other.get_id() == arg.get_id() {
            continue;
        }

        if command
            .get_arg_conflicts_with(other)
            .into_iter()
            .any(|conflict| conflict.get_id() == arg.get_id())
        {
            push_conflict_reference(&mut conflicts, other);
        }
    }

    conflicts.join(", ")
}

fn push_conflict_reference(conflicts: &mut Vec<String>, arg: &Arg) {
    if arg.is_hide_set() {
        return;
    }

    let rendered = render_arg_reference(arg);
    if !conflicts.contains(&rendered) {
        conflicts.push(rendered);
    }
}

fn render_arg_reference(arg: &Arg) -> String {
    if let Some(long) = arg.get_long() {
        return format!("`--{long}`");
    }

    if let Some(short) = arg.get_short() {
        return format!("`-{short}`");
    }

    let name = arg
        .get_value_names()
        .and_then(|value_names| value_names.first())
        .map_or_else(
            || normalize_name(arg.get_id().as_str()),
            |value_name| normalize_name(value_name.as_str()),
        );
    format!("_{name}_")
}

fn render_code_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{
        DOCS_PATH_PREFIX, command_option_meta_map, render_docs_page, strip_leading_section,
    };
    use crate::commands::docgen::command_manuals::COMMAND_MANUALS;
    use clap::{Arg, ArgAction, Command};
    use std::path::Path;

    #[test]
    fn docs_page_includes_frontmatter_and_notice() {
        let spec = COMMAND_MANUALS
            .iter()
            .find(|spec| spec.command == "new")
            .expect("new command manual spec");
        let rendered = render_docs_page(
            spec,
            Path::new("src/doc/man/acton-new.md"),
            "# acton-new(1)\n\n## NAME\n\nacton-new --- Create a new Acton project\n\n## DESCRIPTION\n\nBody.\n",
        );
        assert!(rendered.contains("title: \"acton new\""));
        assert!(rendered.contains("description: \"Reference manual for the acton new command\""));
        assert!(rendered.contains(
            "{/* @generated by `acton docgen`. Do not edit directly. */}\n{/* Source: `src/doc/man/acton-new.md` */}"
        ));
        assert!(!rendered.contains("# acton-new(1)"));
        assert!(!rendered.contains("## NAME\n"));
        assert!(rendered.ends_with("## DESCRIPTION\n\nBody.\n"));
    }

    #[test]
    fn strip_leading_section_removes_only_the_first_matching_section() {
        let body = "## NAME\n\nIntro.\n\n## DESCRIPTION\n\nBody.\n\n## SEE ALSO\n\nMore.\n";

        assert_eq!(
            strip_leading_section(body, "NAME"),
            "## DESCRIPTION\n\nBody.\n\n## SEE ALSO\n\nMore.\n"
        );
    }

    #[test]
    fn strip_leading_section_matches_case_insensitively() {
        let body = "## Name\n\nIntro.\n\n## Description\n\nBody.\n";

        assert_eq!(
            strip_leading_section(body, "NAME"),
            "## Description\n\nBody.\n"
        );
    }

    #[test]
    fn docs_page_rewrites_absolute_docs_links_to_relative_paths() {
        let spec = COMMAND_MANUALS
            .iter()
            .find(|spec| spec.command == "run")
            .expect("run command manual spec");
        let rendered = render_docs_page(
            spec,
            Path::new("src/doc/man/acton-run.md"),
            "## DESCRIPTION\n\n- [Run command guide](https://ton-blockchain.github.io/acton/docs/commands/run)\n",
        );

        assert!(rendered.contains(&format!("]({DOCS_PATH_PREFIX}/commands/run)")));
        assert!(!rendered.contains("https://ton-blockchain.github.io/acton/docs/commands/run"));
    }

    #[test]
    fn builds_option_metadata_from_clap_metadata() {
        let spec = COMMAND_MANUALS
            .iter()
            .find(|spec| spec.command == "check")
            .expect("check command manual spec");
        let mut cli = Command::new("acton").subcommand(
            Command::new("check")
                .arg(
                    Arg::new("format")
                        .long("format")
                        .value_parser(["plain", "json"])
                        .default_value("plain")
                        .conflicts_with("github"),
                )
                .arg(Arg::new("github").long("github").action(ArgAction::SetTrue))
                .arg(
                    Arg::new("include")
                        .long("include")
                        .action(ArgAction::Append),
                ),
        );
        cli.build();

        let option_meta = command_option_meta_map(spec, &cli);
        let format_meta = option_meta
            .get(&("acton check".to_owned(), "--format".to_owned()))
            .expect("format metadata");
        let include_meta = option_meta
            .get(&("acton check".to_owned(), "--include".to_owned()))
            .expect("include metadata");

        assert!(
            format_meta
                .iter()
                .any(|meta| { meta.label == "Possible values" && meta.value == "`plain`, `json`" })
        );
        assert!(
            format_meta
                .iter()
                .any(|meta| meta.label == "Default" && meta.value == "`plain`")
        );
        assert!(
            format_meta
                .iter()
                .any(|meta| meta.label == "Conflicts with" && meta.value == "`--github`")
        );
        assert!(include_meta.iter().any(|meta| meta.label == "Repeatable"));
    }

    #[test]
    fn builds_nested_subcommand_option_metadata() {
        let spec = COMMAND_MANUALS
            .iter()
            .find(|spec| spec.command == "wallet")
            .expect("wallet command manual spec");
        let mut cli = Command::new("acton").subcommand(
            Command::new("wallet").subcommand(
                Command::new("new").arg(
                    Arg::new("version")
                        .long("version")
                        .value_parser(["v5r1", "v4r2"]),
                ),
            ),
        );
        cli.build();

        let option_meta = command_option_meta_map(spec, &cli);
        let version_meta = option_meta
            .get(&("acton wallet new".to_owned(), "--version".to_owned()))
            .expect("nested version metadata");

        assert!(
            version_meta
                .iter()
                .any(|meta| { meta.label == "Possible values" && meta.value == "`v5r1`, `v4r2`" })
        );
    }

    #[test]
    fn maps_option_metadata_by_arg_id_and_aliases() {
        let spec = COMMAND_MANUALS
            .iter()
            .find(|spec| spec.command == "wrapper")
            .expect("wrapper command manual spec");
        let mut cli = Command::new("acton").subcommand(
            Command::new("wrapper")
                .arg(
                    Arg::new("typescript")
                        .long("ts")
                        .conflicts_with_all(["test", "test_output"]),
                )
                .arg(Arg::new("test").long("test").action(ArgAction::SetTrue))
                .arg(Arg::new("test_output").long("test-output")),
        );
        cli.build();

        let option_meta = command_option_meta_map(spec, &cli);
        let by_id = option_meta
            .get(&("acton wrapper".to_owned(), "typescript".to_owned()))
            .expect("metadata by arg id");
        let by_flag = option_meta
            .get(&("acton wrapper".to_owned(), "--ts".to_owned()))
            .expect("metadata by long flag");

        assert_eq!(by_id, by_flag);
        assert!(by_id.iter().any(|meta| {
            meta.label == "Conflicts with" && meta.value == "`--test`, `--test-output`"
        }));
    }
}
