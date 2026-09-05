use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result, ensure};
use clap::Args;
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Args)]
pub struct BuildArgs {
    /// State directory whose tools directory receives sources and build artifacts.
    #[arg(long, default_value = ".localton")]
    pub state_dir: PathBuf,

    /// Installation directory. Defaults to `STATE_DIR/tools/<tool>/install`.
    #[arg(long)]
    pub install_dir: Option<PathBuf>,

    /// Number of parallel native compilation jobs.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=64))]
    pub jobs: u8,

    /// Source repository override.
    #[arg(long)]
    repository: Option<String>,

    /// Pinned source commit override (full SHA).
    #[arg(long)]
    commit: Option<String>,
}

impl BuildArgs {
    pub fn source(
        &self,
        repository: &str,
        commit: &str,
        env_prefix: &str,
    ) -> Result<(String, String)> {
        let repository = self
            .repository
            .clone()
            .or_else(|| nonempty_env(&format!("{env_prefix}_REPOSITORY")))
            .unwrap_or_else(|| repository.to_owned());
        let commit = self
            .commit
            .clone()
            .or_else(|| nonempty_env(&format!("{env_prefix}_COMMIT")))
            .unwrap_or_else(|| commit.to_owned());
        ensure!(
            commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "source commit must be a full 40-character SHA"
        );
        Ok((repository, commit))
    }
}

pub struct Paths {
    pub root: PathBuf,
    pub source: PathBuf,
    pub build: PathBuf,
    pub install: PathBuf,
}

impl Paths {
    pub fn new(
        state_dir: &Path,
        install_dir: Option<&Path>,
        tool: &str,
        build_dir: &str,
    ) -> Result<Self> {
        let root = absolute_path(state_dir)?.join("tools").join(tool);
        Ok(Self {
            source: root.join("source"),
            build: root.join(build_dir),
            install: install_dir
                .map(absolute_path)
                .transpose()?
                .unwrap_or_else(|| root.join("install")),
            root,
        })
    }
}

pub async fn init_checkout(source: &Path, repository: &str) -> Result<()> {
    if !source.exists() {
        run(
            "initialize source checkout",
            Command::new("git").args(["init", "-q"]).arg(source),
        )
        .await?;
    }
    ensure!(
        source.join(".git").exists(),
        "{} exists but is not a Git source checkout",
        source.display()
    );
    run(
        "configure source remote",
        Command::new("git")
            .current_dir(source)
            .args(["config", "remote.origin.url", repository]),
    )
    .await
}

pub async fn head_matches(source: &Path, commit: &str) -> Result<bool> {
    let output = Command::new("git")
        .current_dir(source)
        .args(["rev-parse", "HEAD"])
        .output()
        .await
        .with_context(|| format!("failed to inspect checkout at {}", source.display()))?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == commit)
}

pub async fn checkout_commit(source: &Path, commit: &str) -> Result<()> {
    run(
        "fetch pinned source",
        Command::new("git")
            .current_dir(source)
            .args(["fetch", "--depth", "1", "origin", commit]),
    )
    .await?;
    run(
        "check out pinned source",
        Command::new("git")
            .current_dir(source)
            .args(["checkout", "--detach", "--force", commit]),
    )
    .await
}

pub fn prepend_path(command: &mut Command, name: &str, path: &Path) -> Result<()> {
    let mut paths = vec![path.to_owned()];
    if let Some(existing) = env::var_os(name) {
        paths.extend(env::split_paths(&existing));
    }
    command.env(name, env::join_paths(paths)?);
    Ok(())
}

fn nonempty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

pub fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

pub async fn run(description: &str, command: &mut Command) -> Result<()> {
    info!("{description}");
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .status()
        .await
        .with_context(|| format!("failed to {description}"))?;
    ensure!(status.success(), "{description} failed with {status}");
    Ok(())
}

pub fn copy_executable_atomic(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("installed executable has no parent directory")?;
    let file_name = destination
        .file_name()
        .context("installed executable has no file name")?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));

    let install_result = (|| {
        fs::copy(source, &temporary).with_context(|| {
            format!(
                "failed to copy {} to temporary executable {}",
                source.display(),
                temporary.display()
            )
        })?;
        fs::rename(&temporary, destination).with_context(|| {
            format!(
                "failed to atomically install {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        Ok(())
    })();
    if install_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    install_result
}

pub fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    ensure!(
        source.is_dir(),
        "source directory is missing: {}",
        source.display()
    );
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read {}", source.display()))?;
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}
