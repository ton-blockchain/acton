use clap::Subcommand;
use inquire::Select;
use std::fs;
use std::path::Path;

const DEFAULT_HOOKS_PATH: &str = ".githooks";
const GIT_HOOKS_PATH_KEY: &str = "core.hooksPath";
const PRE_COMMIT_HOOK_FILE: &str = "pre-commit";
const READY_PRE_COMMIT_HOOK: &str = include_str!("templates/.githooks/pre-commit");

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum HooksTemplate {
    Empty,
    Ready,
}

impl HooksTemplate {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Ready => "ready",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Empty => "Create .githooks with an empty pre-commit hook",
            Self::Ready => "Create .githooks with a starter pre-commit hook",
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
    let template = if let Some(template) = template {
        template
    } else {
        Select::new(
            "Hooks template:",
            vec![
                HooksTemplateSelectItem(HooksTemplate::Empty),
                HooksTemplateSelectItem(HooksTemplate::Ready),
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

fn create_hooks_scaffold(template: HooksTemplate) -> anyhow::Result<()> {
    let hooks_dir = Path::new(DEFAULT_HOOKS_PATH);
    if hooks_dir.exists() {
        anyhow::bail!("{} already exists", hooks_dir.display());
    }

    fs::create_dir_all(hooks_dir)?;

    match template {
        HooksTemplate::Empty => {
            write_pre_commit_hook(hooks_dir, "")?;
        }
        HooksTemplate::Ready => {
            write_pre_commit_hook(hooks_dir, READY_PRE_COMMIT_HOOK)?;
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

fn hooks_install_cmd() -> anyhow::Result<()> {
    let output = std::process::Command::new("git")
        .args(["config", GIT_HOOKS_PATH_KEY, DEFAULT_HOOKS_PATH])
        .output()
        .map_err(|err| anyhow::anyhow!("Failed to execute git config command: {err}"))?;

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
    let output = std::process::Command::new("git")
        .args(["config", "--get", GIT_HOOKS_PATH_KEY])
        .output()
        .map_err(|err| anyhow::anyhow!("Failed to execute git config command: {err}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let hooks_path = stdout.trim();

        if hooks_path == DEFAULT_HOOKS_PATH {
            println!(
                "git {} is set to {}",
                GIT_HOOKS_PATH_KEY, DEFAULT_HOOKS_PATH
            );
            return Ok(());
        }

        anyhow::bail!(
            "git {} is set to {}, expected {}",
            GIT_HOOKS_PATH_KEY,
            hooks_path,
            DEFAULT_HOOKS_PATH
        );
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if output.status.code() == Some(1) && stderr.is_empty() {
        anyhow::bail!("git {} is not set", GIT_HOOKS_PATH_KEY);
    }

    if stderr.is_empty() {
        anyhow::bail!(
            "Failed to get git hooks path (exit code: {:?})",
            output.status.code()
        );
    }

    anyhow::bail!("Failed to get git hooks path: {stderr}");
}

fn hooks_uninstall_cmd() -> anyhow::Result<()> {
    let output = std::process::Command::new("git")
        .args(["config", "--unset", GIT_HOOKS_PATH_KEY])
        .output()
        .map_err(|err| anyhow::anyhow!("Failed to execute git config command: {err}"))?;

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
