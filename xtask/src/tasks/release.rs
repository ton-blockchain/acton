use crate::modules::release::{
    ACTON_TOML_PATH, CARGO_LOCK_PATH, CARGO_TOML_PATH, DEFAULT_BRANCH_NAME, DOCS_VERSIONS_PATH,
    GITHUB_REPOSITORY_URL, PACKAGE_JSON_PATH, ReleaseContext, check_current_branch_is_master,
    check_local_master_matches_remote, check_master_github_build_succeeded,
    check_no_uncommitted_changes, check_release_tag_does_not_exist, check_release_version_format,
    confirm_expected_yes, create_release_tag, push_release_commit_and_tag,
    refresh_remote_master_ref,
};
use crate::modules::workflow::{Workflow, WorkflowStep};
use anyhow::{Context, Result, bail};
use clap::Args;
use std::fs;
use std::process::Command;

const CHANGELOG_PATH: &str = "CHANGELOG.md";

#[derive(Args)]
pub(crate) struct ReleaseArgs {
    #[arg(long = "version", value_name = "VERSION")]
    pub(crate) version: String,
}

pub(crate) fn run(args: ReleaseArgs) -> Result<()> {
    let context = ReleaseContext::new(args.version);

    println!(
        "Creating release `{}` from tag `{}`",
        context.version, context.tag
    );

    release_workflow().run(&context)?;

    println!(
        "Release tag pushed successfully. GitHub Actions will publish the GitHub release from tag `{}` at {}/releases/tag/{}",
        context.tag, GITHUB_REPOSITORY_URL, context.tag
    );
    Ok(())
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
                name: "check changelog has release entry",
                run: check_changelog_has_release_entry,
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
                name: "check local master matches remote",
                run: check_local_master_matches_remote,
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
        ],
    }
}

fn check_changelog_has_release_entry(context: &ReleaseContext) -> Result<()> {
    let changelog_contents = fs::read_to_string(CHANGELOG_PATH)
        .with_context(|| format!("failed to read {CHANGELOG_PATH}"))?;

    ensure_changelog_has_release_entry(&changelog_contents, &context.version)
        .with_context(|| format!("failed to validate {CHANGELOG_PATH}"))
}

fn ensure_changelog_has_release_entry(changelog_contents: &str, version: &str) -> Result<()> {
    let changelog =
        parse_changelog::parse(changelog_contents).context("failed to parse changelog")?;

    if changelog.get(version).is_none() {
        bail!("CHANGELOG.md does not contain a release entry for version `{version}`");
    }

    Ok(())
}

fn bump_versions_from_tag(context: &ReleaseContext) -> Result<()> {
    run_yq_update(PACKAGE_JSON_PATH, ".version", &context.version)?;
    update_toml_file(ACTON_TOML_PATH, |document| {
        document["package"]["version"] = toml_edit::value(&context.version);
    })?;
    update_toml_file(CARGO_TOML_PATH, |document| {
        document["workspace"]["package"]["version"] = toml_edit::value(&context.version);
    })?;
    update_docs_versions_file(&context.version)?;
    run_cargo_lock_update()?;

    Ok(())
}

fn create_version_bump_commit(context: &ReleaseContext) -> Result<()> {
    let bump_files = [
        ACTON_TOML_PATH,
        CARGO_TOML_PATH,
        CARGO_LOCK_PATH,
        DOCS_VERSIONS_PATH,
        PACKAGE_JSON_PATH,
    ];

    context.git.add_files(&bump_files)?;
    context.git.commit(
        &format!("chore(acton): bump to version `{}`", context.version),
        &[],
    )
}

fn show_created_commit_numstat(context: &ReleaseContext) -> Result<()> {
    let commit_numstat = context.git.show_commit_numstat("HEAD")?;

    println!("Created commit numstat:\n");
    println!("{commit_numstat}");

    Ok(())
}

fn confirm_release_push(context: &ReleaseContext) -> Result<()> {
    confirm_expected_yes(
        &format!(
            "Type `yes` to push branch `{}` and tag `{}`: ",
            DEFAULT_BRANCH_NAME, context.tag
        ),
        "push aborted: expected `yes` confirmation",
    )
}

fn update_toml_file(path: &str, update: impl FnOnce(&mut toml_edit::DocumentMut)) -> Result<()> {
    let contents = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let mut document = contents
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {path}"))?;

    update(&mut document);

    fs::write(path, document.to_string()).with_context(|| format!("failed to write {path}"))
}

fn update_docs_versions_file(version: &str) -> Result<()> {
    let contents = docs_versions_file_contents(version);
    fs::write(DOCS_VERSIONS_PATH, contents)
        .with_context(|| format!("failed to write {DOCS_VERSIONS_PATH}"))
}

fn docs_versions_file_contents(version: &str) -> String {
    format!("acton_version={version}\n")
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
