use acton_mdman::{Format, ManMap, convert};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const COMMAND_MANUAL_SOURCE_DIR: &str = "src/doc/man";

#[derive(Debug, Clone, Copy)]
pub(super) struct CommandManualSpec {
    pub(super) command: &'static str,
    pub(super) source_name: &'static str,
    pub(super) docs_slug: &'static str,
    pub(super) docs_title: &'static str,
    pub(super) docs_description: &'static str,
}

pub(super) const COMMAND_MANUALS: &[CommandManualSpec] = &[
    CommandManualSpec {
        command: "new",
        source_name: "acton-new.md",
        docs_slug: "new",
        docs_title: "acton new",
        docs_description: "Reference manual for the acton new command",
    },
    CommandManualSpec {
        command: "build",
        source_name: "acton-build.md",
        docs_slug: "build",
        docs_title: "acton build",
        docs_description: "Reference manual for the acton build command",
    },
];

#[derive(Debug, Clone)]
pub(super) struct GeneratedCommandManualPaths {
    pub docs: Vec<PathBuf>,
    pub man: Vec<PathBuf>,
    pub terminal_help: Vec<PathBuf>,
}

pub(super) fn generated_output_paths() -> GeneratedCommandManualPaths {
    GeneratedCommandManualPaths {
        docs: COMMAND_MANUALS
            .iter()
            .map(|spec| PathBuf::from(format!("{}.mdx", spec.docs_slug)))
            .collect(),
        man: COMMAND_MANUALS
            .iter()
            .map(|spec| PathBuf::from(format!("acton-{}.1", spec.command)))
            .collect(),
        terminal_help: COMMAND_MANUALS
            .iter()
            .map(|spec| PathBuf::from(format!("acton-{}.txt", spec.command)))
            .collect(),
    }
}

pub(super) fn generate_command_manual_artifacts(
    man_out_dir: &Path,
    terminal_help_out_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(man_out_dir)?;
    fs::create_dir_all(terminal_help_out_dir)?;

    for spec in COMMAND_MANUALS {
        generate_single_command_manual_artifacts(spec, man_out_dir, terminal_help_out_dir)?;
    }

    Ok(())
}

fn generate_single_command_manual_artifacts(
    spec: &CommandManualSpec,
    man_out_dir: &Path,
    terminal_help_out_dir: &Path,
) -> Result<()> {
    let source_path = Path::new(COMMAND_MANUAL_SOURCE_DIR).join(spec.source_name);
    let man_map = ManMap::new();

    let terminal_help = convert(&source_path, Format::Text, None, man_map.clone())
        .with_context(|| format!("Failed to render text manual {}", source_path.display()))?;
    let man_page = convert(&source_path, Format::Man, None, man_map)
        .with_context(|| format!("Failed to render man page {}", source_path.display()))?;

    fs::write(
        terminal_help_out_dir.join(format!("acton-{}.txt", spec.command)),
        terminal_help,
    )?;
    fs::write(
        man_out_dir.join(format!("acton-{}.1", spec.command)),
        man_page,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{COMMAND_MANUALS, generated_output_paths};

    #[test]
    fn generated_output_paths_match_specs() {
        let paths = generated_output_paths();
        assert_eq!(paths.docs.len(), COMMAND_MANUALS.len());
        assert_eq!(paths.man.len(), COMMAND_MANUALS.len());
        assert_eq!(paths.terminal_help.len(), COMMAND_MANUALS.len());
    }
}
