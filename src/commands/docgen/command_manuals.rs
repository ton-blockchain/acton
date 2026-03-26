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
        command: "init",
        source_name: "acton-init.md",
        docs_slug: "init",
        docs_title: "acton init",
        docs_description: "Reference manual for the acton init command",
    },
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
    CommandManualSpec {
        command: "help",
        source_name: "acton-help.md",
        docs_slug: "help",
        docs_title: "acton help",
        docs_description: "Reference manual for the acton help command",
    },
    CommandManualSpec {
        command: "hooks",
        source_name: "acton-hooks.md",
        docs_slug: "hooks",
        docs_title: "acton hooks",
        docs_description: "Reference manual for the acton hooks command",
    },
    CommandManualSpec {
        command: "compile",
        source_name: "acton-compile.md",
        docs_slug: "compile",
        docs_title: "acton compile",
        docs_description: "Reference manual for the acton compile command",
    },
    CommandManualSpec {
        command: "wrapper",
        source_name: "acton-wrapper.md",
        docs_slug: "wrapper",
        docs_title: "acton wrapper",
        docs_description: "Reference manual for the acton wrapper command",
    },
    CommandManualSpec {
        command: "disasm",
        source_name: "acton-disasm.md",
        docs_slug: "disasm",
        docs_title: "acton disasm",
        docs_description: "Reference manual for the acton disasm command",
    },
    CommandManualSpec {
        command: "fmt",
        source_name: "acton-fmt.md",
        docs_slug: "fmt",
        docs_title: "acton fmt",
        docs_description: "Reference manual for the acton fmt command",
    },
    CommandManualSpec {
        command: "retrace",
        source_name: "acton-retrace.md",
        docs_slug: "retrace",
        docs_title: "acton retrace",
        docs_description: "Reference manual for the acton retrace command",
    },
    CommandManualSpec {
        command: "test",
        source_name: "acton-test.md",
        docs_slug: "test",
        docs_title: "acton test",
        docs_description: "Reference manual for the acton test command",
    },
    CommandManualSpec {
        command: "check",
        source_name: "acton-check.md",
        docs_slug: "check",
        docs_title: "acton check",
        docs_description: "Reference manual for the acton check command",
    },
    CommandManualSpec {
        command: "script",
        source_name: "acton-script.md",
        docs_slug: "script",
        docs_title: "acton script",
        docs_description: "Reference manual for the acton script command",
    },
    CommandManualSpec {
        command: "run",
        source_name: "acton-run.md",
        docs_slug: "run",
        docs_title: "acton run",
        docs_description: "Reference manual for the acton run command",
    },
    CommandManualSpec {
        command: "verify",
        source_name: "acton-verify.md",
        docs_slug: "verify",
        docs_title: "acton verify",
        docs_description: "Reference manual for the acton verify command",
    },
    CommandManualSpec {
        command: "library",
        source_name: "acton-library.md",
        docs_slug: "library",
        docs_title: "acton library",
        docs_description: "Reference manual for the acton library command",
    },
    CommandManualSpec {
        command: "wallet",
        source_name: "acton-wallet.md",
        docs_slug: "wallet",
        docs_title: "acton wallet",
        docs_description: "Reference manual for the acton wallet command",
    },
    CommandManualSpec {
        command: "litenode",
        source_name: "acton-litenode.md",
        docs_slug: "litenode",
        docs_title: "acton litenode",
        docs_description: "Reference manual for the acton litenode command",
    },
    CommandManualSpec {
        command: "doc",
        source_name: "acton-doc.md",
        docs_slug: "doc",
        docs_title: "acton doc",
        docs_description: "Reference manual for the acton doc command",
    },
    CommandManualSpec {
        command: "ls",
        source_name: "acton-ls.md",
        docs_slug: "ls",
        docs_title: "acton ls",
        docs_description: "Reference manual for the acton ls command",
    },
    CommandManualSpec {
        command: "up",
        source_name: "acton-up.md",
        docs_slug: "up",
        docs_title: "acton up",
        docs_description: "Reference manual for the acton up command",
    },
    CommandManualSpec {
        command: "doctor",
        source_name: "acton-doctor.md",
        docs_slug: "doctor",
        docs_title: "acton doctor",
        docs_description: "Reference manual for the acton doctor command",
    },
    CommandManualSpec {
        command: "func2tolk",
        source_name: "acton-func2tolk.md",
        docs_slug: "func2tolk",
        docs_title: "acton func2tolk",
        docs_description: "Reference manual for the acton func2tolk command",
    },
    CommandManualSpec {
        command: "completions",
        source_name: "acton-completions.md",
        docs_slug: "shell-completions",
        docs_title: "acton completions",
        docs_description: "Reference manual for the acton completions command",
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
