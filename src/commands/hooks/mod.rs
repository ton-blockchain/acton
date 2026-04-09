use acton_config::config::project_root as configured_project_root;
use clap::Subcommand;
use inquire::Select;
use path_absolutize::Absolutize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

const DEFAULT_HOOKS_PATH: &str = ".githooks";
const GIT_HOOKS_PATH_KEY: &str = "core.hooksPath";
const PRE_COMMIT_HOOK_FILE: &str = "pre-commit";
const DEFAULT_PRE_COMMIT_HOOK: &str = include_str!("templates/.githooks/pre-commit");
const HOOKS_UNINSTALL_HINT: &str = "Run `acton hooks uninstall` first.";
const HOOKS_NEW_HINT: &str = "Run `acton hooks new` first.";
const HOOKS_REPO_REQUIRED_MESSAGE: &str =
    "Git hooks can only be managed in a project root containing .git. Run `git init` first.";

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
    ensure_local_git_repository_at(
        configured_project_root(),
        "Hooks scaffold can only be created in a project root containing .git. Run `git init` first.",
    )?;

    if let Some(hooks_path) = local_hooks_path_at(configured_project_root())? {
        anyhow::bail!(
            "git {GIT_HOOKS_PATH_KEY} is already set to {hooks_path}. {HOOKS_UNINSTALL_HINT}"
        );
    }

    if let Some(pre_commit_path) = scaffold_pre_commit_path_at(configured_project_root()) {
        anyhow::bail!(
            "Found existing pre-commit hook at {}. Delete it before running `acton hooks new`.",
            pre_commit_path.display()
        );
    }

    if hooks_dir_at(configured_project_root()).exists() {
        anyhow::bail!(
            "Hooks directory {DEFAULT_HOOKS_PATH} already exists. Delete it before running `acton hooks new`."
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

    create_hooks_scaffold_at(configured_project_root(), template)?;

    println!("Created {template} hooks scaffold in {DEFAULT_HOOKS_PATH}");
    println!("Run `acton hooks install` to enable it.");
    Ok(())
}

fn hooks_dir_at(project_root: &Path) -> PathBuf {
    project_root.join(DEFAULT_HOOKS_PATH)
}

fn scaffold_pre_commit_path_at(project_root: &Path) -> Option<PathBuf> {
    let pre_commit_path = hooks_dir_at(project_root).join(PRE_COMMIT_HOOK_FILE);
    if pre_commit_path.exists() {
        Some(pre_commit_path)
    } else {
        None
    }
}

fn has_local_git_repository_at(project_root: &Path) -> bool {
    project_root.join(".git").exists()
}

fn ensure_local_git_repository_at(
    project_root: &Path,
    message: &'static str,
) -> anyhow::Result<()> {
    if has_local_git_repository_at(project_root) {
        Ok(())
    } else {
        anyhow::bail!(message);
    }
}

fn resolve_hooks_path_at(project_root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    if let Ok(canonical_path) = dunce::canonicalize(&absolute) {
        return Ok(canonical_path);
    }

    Ok(absolute.absolutize()?.into_owned())
}

fn hooks_path_matches_default_at(project_root: &Path, hooks_path: &str) -> anyhow::Result<bool> {
    Ok(resolve_hooks_path_at(project_root, Path::new(hooks_path))?
        == resolve_hooks_path_at(project_root, Path::new(DEFAULT_HOOKS_PATH))?)
}

fn create_hooks_scaffold_at(project_root: &Path, template: HooksTemplate) -> anyhow::Result<()> {
    let hooks_dir = hooks_dir_at(project_root);

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

fn git_config_output_at(project_root: &Path, args: &[&str]) -> anyhow::Result<Output> {
    std::process::Command::new("git")
        .args(["config", "--local"])
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(|err| anyhow::anyhow!("Failed to execute git config command: {err}"))
}

fn local_hooks_path_at(project_root: &Path) -> anyhow::Result<Option<String>> {
    let output = git_config_output_at(project_root, &["--get", GIT_HOOKS_PATH_KEY])?;

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
    ensure_local_git_repository_at(configured_project_root(), HOOKS_REPO_REQUIRED_MESSAGE)?;

    if !hooks_dir_at(configured_project_root()).is_dir() {
        anyhow::bail!("Hooks directory {DEFAULT_HOOKS_PATH} does not exist. {HOOKS_NEW_HINT}");
    }

    if let Some(hooks_path) = local_hooks_path_at(configured_project_root())? {
        if hooks_path_matches_default_at(configured_project_root(), &hooks_path)? {
            anyhow::bail!("Git hooks are already installed. {HOOKS_UNINSTALL_HINT}");
        }

        anyhow::bail!(
            "git {GIT_HOOKS_PATH_KEY} is already set to {hooks_path}. {HOOKS_UNINSTALL_HINT}"
        );
    }

    let output = git_config_output_at(
        configured_project_root(),
        &[GIT_HOOKS_PATH_KEY, DEFAULT_HOOKS_PATH],
    )?;
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

        anyhow::bail!("Failed to set git hooks path to {DEFAULT_HOOKS_PATH}: {stderr}");
    }

    println!("Git hooks installed successfully from {DEFAULT_HOOKS_PATH}");

    Ok(())
}

fn hooks_status_cmd() -> anyhow::Result<()> {
    ensure_local_git_repository_at(configured_project_root(), HOOKS_REPO_REQUIRED_MESSAGE)?;

    match local_hooks_path_at(configured_project_root())? {
        Some(hooks_path)
            if hooks_path_matches_default_at(configured_project_root(), &hooks_path)? =>
        {
            println!("Git hooks are installed");
        }
        Some(_) | None => {
            println!("Git hooks are not installed");
        }
    }

    Ok(())
}

fn hooks_uninstall_cmd() -> anyhow::Result<()> {
    ensure_local_git_repository_at(configured_project_root(), HOOKS_REPO_REQUIRED_MESSAGE)?;

    if local_hooks_path_at(configured_project_root())?.is_none() {
        println!("Git hooks are not installed");
        return Ok(());
    }

    let output = git_config_output_at(
        configured_project_root(),
        &["--unset-all", GIT_HOOKS_PATH_KEY],
    )?;

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

pub fn scaffold_and_install_default_hooks(project_root: &Path) -> anyhow::Result<()> {
    ensure_local_git_repository_at(project_root, HOOKS_REPO_REQUIRED_MESSAGE)?;

    if let Some(hooks_path) = local_hooks_path_at(project_root)? {
        if hooks_path_matches_default_at(project_root, &hooks_path)? {
            anyhow::bail!("Git hooks are already installed. {HOOKS_UNINSTALL_HINT}");
        }

        anyhow::bail!(
            "git {GIT_HOOKS_PATH_KEY} is already set to {hooks_path}. {HOOKS_UNINSTALL_HINT}"
        );
    }

    if let Some(pre_commit_path) = scaffold_pre_commit_path_at(project_root) {
        anyhow::bail!(
            "Found existing pre-commit hook at {}. Delete it before enabling default hooks.",
            pre_commit_path.display()
        );
    }

    if hooks_dir_at(project_root).exists() {
        anyhow::bail!(
            "Hooks directory {DEFAULT_HOOKS_PATH} already exists. Delete it before enabling default hooks."
        );
    }

    create_hooks_scaffold_at(project_root, HooksTemplate::Default)?;

    let output = git_config_output_at(project_root, &[GIT_HOOKS_PATH_KEY, DEFAULT_HOOKS_PATH])?;
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

        anyhow::bail!("Failed to set git hooks path to {DEFAULT_HOOKS_PATH}: {stderr}");
    }

    Ok(())
}
