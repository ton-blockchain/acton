use anyhow::{Context, Result, bail};
use clap::Args;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEFAULT_REPOSITORY: &str = "https://github.com/i582/actonscan";
const DEFAULT_BRANCH: &str = "pages";
const DEFAULT_CHECKOUT_DIR: &str = "target/actonscan-pages";
const EXPLORER_PACKAGE: &str = "acton-explorer-ui";

#[derive(Args)]
pub(crate) struct DeployExplorerArgs {
    #[arg(long, value_name = "URL", default_value = DEFAULT_REPOSITORY)]
    repository: String,

    #[arg(long, value_name = "BRANCH", default_value = DEFAULT_BRANCH)]
    branch: String,

    #[arg(long, value_name = "PATH", default_value = DEFAULT_CHECKOUT_DIR)]
    checkout: PathBuf,

    #[arg(long, value_name = "DOMAIN")]
    cname: Option<String>,

    #[arg(long, value_name = "MESSAGE", default_value = "Deploy actonscan")]
    message: String,
}

pub(crate) fn run(args: DeployExplorerArgs) -> Result<()> {
    let workspace_root = workspace_root()?;
    let checkout_dir = resolve_path(&workspace_root, &args.checkout);
    let dist_dir = workspace_root
        .join("crates")
        .join(EXPLORER_PACKAGE)
        .join("dist");

    println!("Building `{EXPLORER_PACKAGE}`");
    run_inherited(
        Command::new("bun")
            .arg("--filter")
            .arg(EXPLORER_PACKAGE)
            .arg("build")
            .current_dir(&workspace_root),
    )?;

    ensure_dist_ready(&dist_dir)?;
    ensure_checkout(&checkout_dir, &args.repository)?;
    prepare_branch(&checkout_dir, &args.branch)?;
    sync_dist(&dist_dir, &checkout_dir, args.cname.as_deref())?;
    commit_and_push(&checkout_dir, &args.branch, &args.message)?;

    println!(
        "Explorer deploy pushed to `{}` branch `{}`",
        args.repository, args.branch
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("failed to resolve workspace root from xtask manifest directory")
}

fn resolve_path(workspace_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

fn ensure_dist_ready(dist_dir: &Path) -> Result<()> {
    let index_html = dist_dir.join("index.html");
    if !index_html.is_file() {
        bail!(
            "explorer build output is missing `{}`",
            index_html.display()
        );
    }

    Ok(())
}

fn ensure_checkout(checkout_dir: &Path, repository: &str) -> Result<()> {
    if checkout_dir.join(".git").is_dir() {
        println!(
            "Using existing deploy checkout `{}`",
            checkout_dir.display()
        );
        return Ok(());
    }

    if checkout_dir.exists() {
        bail!(
            "deploy checkout path `{}` exists but is not a git repository",
            checkout_dir.display()
        );
    }

    if let Some(parent) = checkout_dir.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }

    println!("Cloning `{repository}` into `{}`", checkout_dir.display());
    run_inherited(
        Command::new("git")
            .arg("clone")
            .arg(repository)
            .arg(checkout_dir),
    )
}

fn prepare_branch(checkout_dir: &Path, branch: &str) -> Result<()> {
    run_inherited(git(checkout_dir).arg("fetch").arg("origin"))?;

    if remote_branch_exists(checkout_dir, branch)? {
        println!("Checking out existing deploy branch `{branch}`");
        run_inherited(git(checkout_dir).arg("checkout").arg(branch))?;
        run_inherited(
            git(checkout_dir)
                .arg("reset")
                .arg("--hard")
                .arg(format!("origin/{branch}")),
        )?;
    } else {
        println!("Creating orphan deploy branch `{branch}`");
        run_inherited(
            git(checkout_dir)
                .arg("checkout")
                .arg("--orphan")
                .arg(branch),
        )?;
    }

    clean_checkout_contents(checkout_dir)
}

fn remote_branch_exists(checkout_dir: &Path, branch: &str) -> Result<bool> {
    let output = git(checkout_dir)
        .arg("ls-remote")
        .arg("--heads")
        .arg("origin")
        .arg(branch)
        .output()
        .with_context(|| format!("failed to check remote branch `{branch}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git ls-remote failed with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    Ok(!output.stdout.is_empty())
}

fn clean_checkout_contents(checkout_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(checkout_dir)
        .with_context(|| format!("failed to read `{}`", checkout_dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in `{}`", checkout_dir.display()))?;
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }

        remove_path(&entry.path())?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat `{}`", path.display()))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory `{}`", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove `{}`", path.display()))?;
    }

    Ok(())
}

fn sync_dist(dist_dir: &Path, checkout_dir: &Path, cname: Option<&str>) -> Result<()> {
    copy_dir_recursive(dist_dir, checkout_dir)?;
    fs::write(checkout_dir.join(".nojekyll"), "").with_context(|| {
        format!(
            "failed to write `{}`",
            checkout_dir.join(".nojekyll").display()
        )
    })?;

    if let Some(cname) = cname.map(str::trim).filter(|value| !value.is_empty()) {
        fs::write(checkout_dir.join("CNAME"), format!("{cname}\n")).with_context(|| {
            format!("failed to write `{}`", checkout_dir.join("CNAME").display())
        })?;
    }

    Ok(())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("failed to create `{}`", to.display()))?;

    for entry in
        fs::read_dir(from).with_context(|| format!("failed to read `{}`", from.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", from.display()))?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for `{}`", from_path.display()))?;

        if file_type.is_dir() {
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            fs::copy(&from_path, &to_path).with_context(|| {
                format!(
                    "failed to copy `{}` to `{}`",
                    from_path.display(),
                    to_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn commit_and_push(checkout_dir: &Path, branch: &str, message: &str) -> Result<()> {
    run_inherited(git(checkout_dir).arg("add").arg("-A"))?;

    if !has_staged_changes(checkout_dir)? {
        println!("Deploy checkout has no changes; skipping commit and push");
        return Ok(());
    }

    run_inherited(git(checkout_dir).arg("commit").arg("-m").arg(message))?;
    run_inherited(
        git(checkout_dir)
            .arg("push")
            .arg("origin")
            .arg(format!("HEAD:{branch}")),
    )
}

fn has_staged_changes(checkout_dir: &Path) -> Result<bool> {
    let status = git(checkout_dir)
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .status()
        .context("failed to run git diff --cached --quiet")?;

    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => bail!("git diff --cached --quiet failed with status {status}"),
    }
}

fn git(checkout_dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(checkout_dir);
    command
}

fn run_inherited(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to run `{program} {args}`"))?;

    if !status.success() {
        bail!("`{program} {args}` failed with status {status}");
    }

    Ok(())
}
