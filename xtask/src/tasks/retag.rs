use crate::modules::release::{
    ACTON_TOML_PATH, CARGO_TOML_PATH, DEFAULT_BRANCH_NAME, GITHUB_REPOSITORY_URL,
    ORIGIN_REMOTE_NAME, PACKAGE_JSON_PATH, ReleaseContext, check_current_branch_is_master,
    check_github_release_does_not_exist, check_local_master_matches_remote,
    check_master_github_build_succeeded, check_no_uncommitted_changes, check_release_tag_exists,
    check_release_version_format, confirm_expected_yes, create_release_tag,
    push_release_commit_and_tag, refresh_remote_master_ref,
};
use crate::modules::workflow::{Workflow, WorkflowStep};
use anyhow::{Context, Result, bail};
use clap::Args;
use std::fs;

#[derive(Args)]
pub(crate) struct RetagArgs {
    #[arg(long = "version", value_name = "VERSION")]
    pub(crate) version: String,
}

pub(crate) fn run(args: RetagArgs) -> Result<()> {
    let context = ReleaseContext::new(args.version);

    println!(
        "Retagging `{}` by creating an empty commit on current `{}` HEAD",
        context.tag, DEFAULT_BRANCH_NAME
    );

    retag_workflow().run(&context)?;

    println!(
        "Release retry commit and tag pushed successfully. GitHub Actions will publish the GitHub release from tag `{}` at {}/releases/tag/{}",
        context.tag, GITHUB_REPOSITORY_URL, context.tag
    );
    Ok(())
}

fn retag_workflow() -> Workflow<'static, ReleaseContext> {
    Workflow {
        name: "retag",
        steps: &[
            WorkflowStep {
                name: "check release version format",
                run: check_release_version_format,
            },
            WorkflowStep {
                name: "check project versions match release version",
                run: check_project_versions_match_tag,
            },
            WorkflowStep {
                name: "check release tag exists in origin",
                run: check_release_tag_exists,
            },
            WorkflowStep {
                name: "check GitHub release does not exist",
                run: check_github_release_does_not_exist,
            },
            WorkflowStep {
                name: "check Release workflow failed for current tag",
                run: check_release_workflow_failed_for_current_tag,
            },
            WorkflowStep {
                name: "current branch is master",
                run: check_current_branch_is_master,
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
                name: "create empty retag commit",
                run: create_empty_retag_commit,
            },
            WorkflowStep {
                name: "delete local release tag if it exists",
                run: delete_local_release_tag_if_exists,
            },
            WorkflowStep {
                name: "create release tag",
                run: create_release_tag,
            },
            WorkflowStep {
                name: "confirm release push",
                run: confirm_release_push,
            },
            WorkflowStep {
                name: "delete remote release tag",
                run: delete_remote_release_tag,
            },
            WorkflowStep {
                name: "push release commit and tag",
                run: push_release_commit_and_tag,
            },
        ],
    }
}

fn check_release_workflow_failed_for_current_tag(context: &ReleaseContext) -> Result<()> {
    context
        .github
        .ensure_latest_release_workflow_failed(&context.tag)
}

fn check_project_versions_match_tag(context: &ReleaseContext) -> Result<()> {
    let expected_version = &context.version;

    let acton_toml_version = read_toml_version(ACTON_TOML_PATH, &["package", "version"])?;
    ensure_version_matches_tag(
        ACTON_TOML_PATH,
        "package.version",
        &acton_toml_version,
        expected_version,
    )?;

    let cargo_toml_version =
        read_toml_version(CARGO_TOML_PATH, &["workspace", "package", "version"])?;
    ensure_version_matches_tag(
        CARGO_TOML_PATH,
        "workspace.package.version",
        &cargo_toml_version,
        expected_version,
    )?;

    let package_json_version = read_package_json_version(PACKAGE_JSON_PATH)?;
    ensure_version_matches_tag(
        PACKAGE_JSON_PATH,
        "version",
        &package_json_version,
        expected_version,
    )?;

    Ok(())
}

fn create_empty_retag_commit(context: &ReleaseContext) -> Result<()> {
    context.git.commit(
        &format!("chore(acton): re-bump to version {}", context.version),
        &["--allow-empty"],
    )
}

fn delete_local_release_tag_if_exists(context: &ReleaseContext) -> Result<()> {
    if context.git.local_tag_exists(&context.tag)? {
        context.git.delete_tag(&context.tag)?;
    }

    Ok(())
}

fn delete_remote_release_tag(context: &ReleaseContext) -> Result<()> {
    context
        .git
        .delete_remote_tag(ORIGIN_REMOTE_NAME, &context.tag)
}

fn confirm_release_push(context: &ReleaseContext) -> Result<()> {
    confirm_expected_yes(
        &format!(
            "Type `yes` to push branch `{}` and tag `{}` to `{}`: ",
            DEFAULT_BRANCH_NAME, context.tag, ORIGIN_REMOTE_NAME
        ),
        "push aborted: expected `yes` confirmation",
    )
}

fn read_toml_version(path: &str, key_path: &[&str]) -> Result<String> {
    let contents = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let document = contents
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {path}"))?;

    let mut item = document.as_item();
    for key in key_path {
        item = item
            .get(*key)
            .with_context(|| format!("missing `{}` in {path}", key_path.join(".")))?;
    }

    item.as_str()
        .map(str::to_owned)
        .with_context(|| format!("`{}` in {path} is not a string", key_path.join(".")))
}

fn read_package_json_version(path: &str) -> Result<String> {
    let contents = fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let package_json = serde_json::from_str::<serde_json::Value>(&contents)
        .with_context(|| format!("failed to parse {path}"))?;

    package_json
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .with_context(|| format!("missing string `version` in {path}"))
}

fn ensure_version_matches_tag(
    path: &str,
    field_name: &str,
    actual_version: &str,
    expected_version: &str,
) -> Result<()> {
    if actual_version != expected_version {
        bail!(
            "{path} `{field_name}` version `{actual_version}` does not match release version `{expected_version}`"
        );
    }

    Ok(())
}
