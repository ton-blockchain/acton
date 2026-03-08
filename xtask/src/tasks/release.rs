use std::fs;
use std::io::{self, Write};
use std::process::Command;

use crate::modules::git::Git;
use crate::modules::github::Github;
use crate::modules::workflow::{Workflow, WorkflowStep};
use anyhow::{Context, Result, bail};
use clap::Args;

const DEFAULT_BRANCH_NAME: &str = "master";
const ORIGIN_REMOTE_NAME: &str = "origin";
const GITHUB_REPOSITORY_URL: &str = "https://github.com/i582/acton";

const ACTON_TOML_PATH: &str = "Acton.toml";
const CARGO_TOML_PATH: &str = "Cargo.toml";
const CARGO_LOCK_PATH: &str = "Cargo.lock";
const PACKAGE_JSON_PATH: &str = "package.json";

#[derive(Args)]
pub(crate) struct ReleaseArgs {
    #[arg(long = "version", value_name = "VERSION")]
    pub(crate) version: String,
}

pub(crate) fn run(args: ReleaseArgs) -> Result<()> {
    let context = ReleaseContext::new(args);

    println!(
        "Creating release `{}` from tag `{}`",
        context.version, context.tag
    );

    release_workflow().run(&context)?;

    println!(
        "GitHub release created successfully: {}/releases/tag/{}",
        GITHUB_REPOSITORY_URL, context.tag
    );
    Ok(())
}

struct ReleaseContext {
    github: Github,
    git: Git,
    version: String,
    tag: String,
}

impl ReleaseContext {
    fn new(args: ReleaseArgs) -> Self {
        let version = args.version;
        let tag = format!("v{version}");

        Self {
            github: Github::new(),
            git: Git::new(),
            version,
            tag,
        }
    }
}

fn release_workflow() -> Workflow<'static, ReleaseContext> {
    Workflow {
        name: "release",
        steps: &[
            WorkflowStep {
                name: "check release version format",
                run: check_release_version_format,
            },
            WorkflowStep {
                name: "current branch is master",
                run: check_current_branch_is_master,
            },
            WorkflowStep {
                name: "check release tag does not exist",
                run: check_release_tag_does_not_exist,
            },
            WorkflowStep {
                name: "check no uncommitted changes",
                run: check_no_uncommitted_changes,
            },
            WorkflowStep {
                name: "refresh remote master ref",
                run: refresh_remote_master_ref,
            },
            WorkflowStep {
                name: "check local master is up to date",
                run: check_local_master_is_up_to_date,
            },
            WorkflowStep {
                name: "check master GitHub build succeeded",
                run: check_master_github_build_succeeded,
            },
            WorkflowStep {
                name: "bump versions from tag",
                run: bump_versions_from_tag,
            },
            WorkflowStep {
                name: "create version bump commit",
                run: create_version_bump_commit,
            },
            WorkflowStep {
                name: "create release tag",
                run: create_release_tag,
            },
            WorkflowStep {
                name: "show created commit numstat",
                run: show_created_commit_numstat,
            },
            WorkflowStep {
                name: "confirm release push",
                run: confirm_release_push,
            },
            WorkflowStep {
                name: "push release commit and tag",
                run: push_release_commit_and_tag,
            },
            WorkflowStep {
                name: "create GitHub release",
                run: create_github_release,
            },
        ],
    }
}

fn check_release_version_format(context: &ReleaseContext) -> Result<()> {
    let parts: Vec<&str> = context.version.split('.').collect();
    if parts.len() != 3 {
        bail!(
            "release version `{}` must have three dot-separated parts",
            context.version
        );
    }

    let has_no_empty_parts = parts.iter().all(|part| !part.is_empty());
    if !has_no_empty_parts {
        bail!(
            "release version `{}` must not contain empty version parts",
            context.version
        );
    }

    let has_only_numeric_parts = parts
        .iter()
        .all(|part| part.chars().all(|char| char.is_ascii_digit()));
    if !has_only_numeric_parts {
        bail!(
            "release version `{}` must contain only numeric version parts",
            context.version
        );
    }

    Ok(())
}

fn check_current_branch_is_master(context: &ReleaseContext) -> Result<()> {
    let current_branch = context.git.current_branch()?;

    if current_branch != DEFAULT_BRANCH_NAME {
        bail!("release must be run from the `master` branch, current branch: `{current_branch}`");
    }

    Ok(())
}

fn check_no_uncommitted_changes(context: &ReleaseContext) -> Result<()> {
    if context.git.has_uncommitted_changes()? {
        bail!("`{DEFAULT_BRANCH_NAME}` branch has uncommitted changes");
    }

    Ok(())
}

fn refresh_remote_master_ref(context: &ReleaseContext) -> Result<()> {
    context
        .git
        .fetch_branch(ORIGIN_REMOTE_NAME, DEFAULT_BRANCH_NAME)
}

fn check_local_master_is_up_to_date(context: &ReleaseContext) -> Result<()> {
    let remote_branch = format!("{ORIGIN_REMOTE_NAME}/{DEFAULT_BRANCH_NAME}");
    let behind = context.git.commit_count_between("HEAD", &remote_branch)?;

    if behind != 0 {
        let message = format!("local `master` is behind `{remote_branch}` by {behind} commit(s)");
        bail!(message);
    }

    Ok(())
}

fn check_release_tag_does_not_exist(context: &ReleaseContext) -> Result<()> {
    if context
        .git
        .remote_tag_exists(ORIGIN_REMOTE_NAME, &context.tag)?
    {
        bail!("release tag `{}` already exists on remote", context.tag);
    }

    Ok(())
}

fn check_master_github_build_succeeded(context: &ReleaseContext) -> Result<()> {
    let branch = context.git.current_branch()?;
    let head_sha = context.git.head_commit()?;

    context
        .github
        .ensure_branch_builds_succeeded(&branch, &head_sha)
}

fn bump_versions_from_tag(context: &ReleaseContext) -> Result<()> {
    run_yq_update(PACKAGE_JSON_PATH, ".version", &context.version)?;
    update_toml_file(ACTON_TOML_PATH, |document| {
        document["package"]["version"] = toml_edit::value(&context.version);
    })?;
    update_toml_file(CARGO_TOML_PATH, |document| {
        document["workspace"]["package"]["version"] = toml_edit::value(&context.version);
    })?;
    run_cargo_lock_update()?;

    Ok(())
}

fn create_version_bump_commit(context: &ReleaseContext) -> Result<()> {
    let bump_files = [
        ACTON_TOML_PATH,
        CARGO_TOML_PATH,
        CARGO_LOCK_PATH,
        PACKAGE_JSON_PATH,
    ];

    context.git.add_files(&bump_files)?;
    context.git.commit(&format!(
        "chore(acton): bump to version `{}`",
        context.version
    ))
}

fn create_release_tag(context: &ReleaseContext) -> Result<()> {
    context.git.tag(&context.tag)
}

fn show_created_commit_numstat(context: &ReleaseContext) -> Result<()> {
    let commit_numstat = context.git.show_commit_numstat("HEAD")?;

    println!("Created commit numstat:\n");
    println!("{}", commit_numstat);

    Ok(())
}

fn confirm_release_push(context: &ReleaseContext) -> Result<()> {
    print!(
        "Type `yes` to push branch `{}` and tag `{}`: ",
        DEFAULT_BRANCH_NAME, context.tag
    );
    io::stdout()
        .flush()
        .context("failed to flush confirmation prompt")?;

    let mut confirmation = String::new();
    io::stdin()
        .read_line(&mut confirmation)
        .context("failed to read push confirmation")?;

    if confirmation.trim() != "yes" {
        bail!("push aborted: expected `yes` confirmation");
    }

    Ok(())
}

fn push_release_commit_and_tag(context: &ReleaseContext) -> Result<()> {
    context
        .git
        .push_refs(ORIGIN_REMOTE_NAME, &[DEFAULT_BRANCH_NAME, &context.tag])
}

fn create_github_release(context: &ReleaseContext) -> Result<()> {
    context
        .github
        .create_release(&context.tag, DEFAULT_BRANCH_NAME)
}

fn update_toml_file(path: &str, update: impl FnOnce(&mut toml_edit::DocumentMut)) -> Result<()> {
    let contents = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let mut document = contents
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {path}"))?;

    update(&mut document);

    fs::write(path, document.to_string()).with_context(|| format!("failed to write {path}"))
}

fn run_yq_update(path: &str, field: &str, version: &str) -> Result<()> {
    let expression = format!(r#"{field} = "{version}""#);
    let output = Command::new("yq")
        .args(["-i", &expression, path])
        .output()
        .with_context(|| format!("failed to run yq for {path}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

        bail!("yq failed for {path}: {stderr}");
    }

    Ok(())
}

fn run_cargo_lock_update() -> Result<()> {
    let output = Command::new("cargo")
        .args(["update", "--workspace"])
        .output()
        .context("failed to run cargo update --workspace")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

        bail!("cargo update --workspace failed: {stderr}");
    }

    Ok(())
}
