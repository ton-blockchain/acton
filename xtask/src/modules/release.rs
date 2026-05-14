use std::io::{self, Write};

use crate::modules::git::Git;
use crate::modules::github::Github;
use anyhow::{Context, Result, bail};

pub(crate) const DEFAULT_BRANCH_NAME: &str = "master";
pub(crate) const ORIGIN_REMOTE_NAME: &str = "origin";
pub(crate) const GITHUB_REPOSITORY_URL: &str = "https://github.com/ton-blockchain/acton";

pub(crate) const ACTON_TOML_PATH: &str = "Acton.toml";
pub(crate) const CARGO_TOML_PATH: &str = "Cargo.toml";
pub(crate) const CARGO_LOCK_PATH: &str = "Cargo.lock";
pub(crate) const DOCS_VERSIONS_PATH: &str = "docs/.versions";
pub(crate) const PACKAGE_JSON_PATH: &str = "package.json";

pub(crate) struct ReleaseContext {
    pub(crate) github: Github,
    pub(crate) git: Git,
    pub(crate) version: String,
    pub(crate) tag: String,
}

impl ReleaseContext {
    pub(crate) fn new(version: String) -> Self {
        let tag = format!("v{version}");

        Self {
            github: Github::new(),
            git: Git::new(),
            version,
            tag,
        }
    }
}

pub(crate) fn check_release_version_format(context: &ReleaseContext) -> Result<()> {
    let version = &context.version;

    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        bail!("release version `{version}` must have three dot-separated parts");
    }

    let has_no_empty_parts = parts.iter().all(|part| !part.is_empty());
    if !has_no_empty_parts {
        bail!("release version `{version}` must not contain empty version parts");
    }

    let has_only_numeric_parts = parts
        .iter()
        .all(|part| part.chars().all(|char| char.is_ascii_digit()));
    if !has_only_numeric_parts {
        bail!("release version `{version}` must contain only numeric version parts");
    }

    Ok(())
}

pub(crate) fn check_current_branch_is_master(context: &ReleaseContext) -> Result<()> {
    let current_branch = context.git.current_branch()?;

    if current_branch != DEFAULT_BRANCH_NAME {
        bail!("current branch must be `{DEFAULT_BRANCH_NAME}`, got `{current_branch}`");
    }

    Ok(())
}

pub(crate) fn check_no_uncommitted_changes(context: &ReleaseContext) -> Result<()> {
    if context.git.has_uncommitted_changes()? {
        bail!("`{DEFAULT_BRANCH_NAME}` branch has uncommitted changes");
    }

    Ok(())
}

pub(crate) fn refresh_remote_master_ref(context: &ReleaseContext) -> Result<()> {
    context
        .git
        .fetch_branch(ORIGIN_REMOTE_NAME, DEFAULT_BRANCH_NAME)
}

pub(crate) fn check_local_master_matches_remote(context: &ReleaseContext) -> Result<()> {
    let remote_branch = format!("{ORIGIN_REMOTE_NAME}/{DEFAULT_BRANCH_NAME}");
    let (ahead, behind) = context.git.commit_count_between("HEAD", &remote_branch)?;

    match (ahead, behind) {
        (0, 0) => Ok(()),
        (0, behind) => {
            bail!("local `master` is behind `{remote_branch}` by {behind} commit(s)")
        }
        (ahead, 0) => bail!("local `master` is ahead of `{remote_branch}` by {ahead} commit(s)"),
        (ahead, behind) => bail!(
            "local `master` has diverged from `{remote_branch}` by {ahead} ahead and {behind} behind commit(s)"
        ),
    }
}

pub(crate) fn check_release_tag_does_not_exist(context: &ReleaseContext) -> Result<()> {
    if context
        .git
        .remote_tag_exists(ORIGIN_REMOTE_NAME, &context.tag)?
    {
        bail!("release tag `{}` already exists on remote", context.tag);
    }

    Ok(())
}

pub(crate) fn check_release_tag_exists(context: &ReleaseContext) -> Result<()> {
    if !context
        .git
        .remote_tag_exists(ORIGIN_REMOTE_NAME, &context.tag)?
    {
        bail!("release tag `{}` does not exist on remote", context.tag);
    }

    Ok(())
}

pub(crate) fn check_github_release_does_not_exist(context: &ReleaseContext) -> Result<()> {
    context.github.ensure_release_does_not_exist(&context.tag)
}

pub(crate) fn check_master_github_build_succeeded(context: &ReleaseContext) -> Result<()> {
    let branch = context.git.current_branch()?;
    let head_sha = context.git.head_commit()?;

    context
        .github
        .ensure_branch_builds_succeeded(&branch, &head_sha)
}

pub(crate) fn create_release_tag(context: &ReleaseContext) -> Result<()> {
    context.git.tag(&context.tag)
}

pub(crate) fn push_release_commit_and_tag(context: &ReleaseContext) -> Result<()> {
    context
        .git
        .push_refs(ORIGIN_REMOTE_NAME, &[DEFAULT_BRANCH_NAME, &context.tag])
}

pub(crate) fn confirm_expected_yes(prompt: &str, abort_message: &str) -> Result<()> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .context("failed to flush confirmation prompt")?;

    let mut confirmation = String::new();
    io::stdin()
        .read_line(&mut confirmation)
        .context("failed to read confirmation")?;

    if confirmation.trim() != "yes" {
        bail!("{abort_message}");
    }

    Ok(())
}
