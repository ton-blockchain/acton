use acton_config::config::project_root as configured_project_root;
use clap::Subcommand;
use inquire::Select;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

const DEFAULT_HOOKS_PATH: &str = ".githooks";
const GIT_HOOKS_PATH_KEY: &str = "core.hooksPath";
const PRE_COMMIT_HOOK_FILE: &str = "pre-commit";
const DEFAULT_PRE_COMMIT_HOOK: &str = include_str!("templates/.githooks/pre-commit");
const HOOKS_UNINSTALL_HINT: &str = "Run `acton hooks uninstall` first.";

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum HooksTemplate {
    Empty,
    Default,
}

impl HooksTemplate {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Default => "default",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Empty => "Create .githooks with an empty pre-commit hook",
            Self::Default => "Create .githooks with a starter pre-commit hook",
        }
    }
}

impl std::fmt::Display for HooksTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy)]
struct HooksTemplateSelectItem(HooksTemplate);

impl std::fmt::Display for HooksTemplateSelectItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<8} {}", self.0.as_str(), self.0.description(),)
    }
}

#[derive(Subcommand, Clone)]
pub enum HooksCommand {
    #[command(about = "Create a project hook scaffold")]
    New {
        #[arg(long, value_enum, help = "Hooks scaffold to create")]
        template: Option<HooksTemplate>,
    },
    #[command(about = "Set git core.hooksPath to .githooks")]
    Install,
    #[command(about = "Check whether git core.hooksPath points to .githooks")]
    Status,
    #[command(about = "Unset git core.hooksPath")]
    Uninstall,
}

pub fn hooks_cmd(command: HooksCommand) -> anyhow::Result<()> {
    match command {
        HooksCommand::New { template } => hooks_new_cmd(template),
        HooksCommand::Install => hooks_install_cmd(),
        HooksCommand::Status => hooks_status_cmd(),
        HooksCommand::Uninstall => hooks_uninstall_cmd(),
    }
}

fn hooks_new_cmd(template: Option<HooksTemplate>) -> anyhow::Result<()> {
    if has_local_git_repository()
        && let Some(hooks_path) = local_hooks_path()?
    {
        anyhow::bail!(
            "git {GIT_HOOKS_PATH_KEY} is already set to {hooks_path}. {HOOKS_UNINSTALL_HINT}"
        );
    }

    let template = if let Some(template) = template {
        template
    } else {
        Select::new(
            "Hooks template:",
            vec![
                HooksTemplateSelectItem(HooksTemplate::Default),
                HooksTemplateSelectItem(HooksTemplate::Empty),
            ],
        )
        .with_starting_cursor(0)
        .prompt()?
        .0
    };

    create_hooks_scaffold(template)?;

    println!(
        "Created {} hooks scaffold in {}",
        template, DEFAULT_HOOKS_PATH
    );
    println!("Run `acton hooks install` to enable it.");
    Ok(())
}

fn hooks_dir() -> PathBuf {
    configured_project_root().join(DEFAULT_HOOKS_PATH)
}

fn has_local_git_repository() -> bool {
    configured_project_root().join(".git").exists()
}

fn create_hooks_scaffold(template: HooksTemplate) -> anyhow::Result<()> {
    let hooks_dir = hooks_dir();
    if hooks_dir.exists() {
        anyhow::bail!("{DEFAULT_HOOKS_PATH} already exists");
    }

    fs::create_dir_all(&hooks_dir)?;

    match template {
        HooksTemplate::Empty => {
            write_pre_commit_hook(&hooks_dir, "")?;
        }
        HooksTemplate::Default => {
            write_pre_commit_hook(&hooks_dir, DEFAULT_PRE_COMMIT_HOOK)?;
        }
    }

    Ok(())
}

fn write_pre_commit_hook(hooks_dir: &Path, contents: &str) -> anyhow::Result<()> {
    let pre_commit_path = hooks_dir.join(PRE_COMMIT_HOOK_FILE);
    fs::write(&pre_commit_path, contents)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&pre_commit_path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(pre_commit_path, permissions)?;
    }

    Ok(())
}

fn git_config_output(args: &[&str]) -> anyhow::Result<Output> {
    std::process::Command::new("git")
        .args(["config", "--local"])
        .args(args)
        .current_dir(configured_project_root())
        .output()
        .map_err(|err| anyhow::anyhow!("Failed to execute git config command: {err}"))
}

fn local_hooks_path() -> anyhow::Result<Option<String>> {
    let output = git_config_output(&["--get", GIT_HOOKS_PATH_KEY])?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Ok(Some(stdout.trim().to_owned()));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if output.status.code() == Some(1) && stderr.is_empty() {
        return Ok(None);
    }

    if stderr.is_empty() {
        anyhow::bail!(
            "Failed to get git hooks path (exit code: {:?})",
            output.status.code()
        );
    }

    anyhow::bail!("Failed to get git hooks path: {stderr}");
}

fn hooks_install_cmd() -> anyhow::Result<()> {
    if let Some(hooks_path) = local_hooks_path()? {
        if hooks_path == DEFAULT_HOOKS_PATH {
            anyhow::bail!("Git hooks are already installed. {HOOKS_UNINSTALL_HINT}");
        }

        anyhow::bail!(
            "git {GIT_HOOKS_PATH_KEY} is already set to {hooks_path}. {HOOKS_UNINSTALL_HINT}"
        );
    }

    let output = git_config_output(&[GIT_HOOKS_PATH_KEY, DEFAULT_HOOKS_PATH])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            anyhow::bail!(
                "Failed to set git hooks path to {} (exit code: {:?})",
                DEFAULT_HOOKS_PATH,
                output.status.code(),
            );
        }

        anyhow::bail!(
            "Failed to set git hooks path to {}: {stderr}",
            DEFAULT_HOOKS_PATH
        );
    }

    println!(
        "Git hooks installed successfully from {}",
        DEFAULT_HOOKS_PATH
    );

    Ok(())
}

fn hooks_status_cmd() -> anyhow::Result<()> {
    match local_hooks_path()? {
        Some(hooks_path) if hooks_path == DEFAULT_HOOKS_PATH => {
            println!("Git hooks are installed");
        }
        Some(_) | None => {
            println!("Git hooks are not installed");
        }
    }

    Ok(())
}

fn hooks_uninstall_cmd() -> anyhow::Result<()> {
    let output = git_config_output(&["--unset", GIT_HOOKS_PATH_KEY])?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            anyhow::bail!(
                "Failed to unset git hooks path (exit code: {:?})",
                output.status.code()
            );
        }

        anyhow::bail!("Failed to unset git hooks path: {stderr}");
    }

    println!("Git hooks uninstalled successfully");

    Ok(())
}
