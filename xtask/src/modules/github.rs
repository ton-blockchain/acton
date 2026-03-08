use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Default)]
pub(crate) struct Github;

impl Github {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn create_release(&self, tag: &str, target: &str) -> Result<()> {
        self.command_output(&[
            "release",
            "create",
            tag,
            "--title",
            tag,
            "--latest",
            "--target",
            target,
            "--verify-tag",
            "--generate-notes",
        ])
        .map(|_| ())
    }

    pub(crate) fn ensure_branch_builds_succeeded(
        &self,
        branch: &str,
        head_sha: &str,
    ) -> Result<()> {
        let runs = self.json_output::<Vec<WorkflowRun>>(&[
            "run",
            "list",
            "--branch",
            branch,
            "--commit",
            head_sha,
            "--limit",
            "100",
            "--json",
            "workflowName,status,conclusion",
        ])?;

        if runs.is_empty() {
            bail!("no GitHub Actions runs found for branch `{branch}` and commit `{head_sha}`");
        }

        let failures = runs
            .iter()
            .filter_map(|run| {
                self.ensure_workflow_runs_succeeded(run)
                    .err()
                    .map(|error| error.to_string())
            })
            .collect::<Vec<_>>();

        if !failures.is_empty() {
            bail!(
                "GitHub Actions runs for branch `{branch}` and commit `{head_sha}` are not all successful: {}",
                failures.join("; ")
            );
        }

        Ok(())
    }

    fn ensure_workflow_runs_succeeded(&self, run: &WorkflowRun) -> Result<()> {
        let workflow_name = run.workflow_name.as_deref().unwrap_or("<unnamed workflow>");

        if run.status != "completed" {
            bail!("workflow `{workflow_name}` is not completed");
        }

        if run.conclusion.as_deref() != Some("success") {
            bail!("workflow `{workflow_name}` did not succeed");
        }

        Ok(())
    }

    fn json_output<T>(&self, args: &[&str]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let output = self.command_output(args)?;

        serde_json::from_slice(&output.stdout)
            .with_context(|| format!("failed to parse JSON from gh {}", args.join(" ")))
    }

    fn command_output(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("gh")
            .args(args)
            .output()
            .with_context(|| format!("failed to run gh {}", args.join(" ")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

            bail!(
                "gh {} failed with status {}: {}",
                args.join(" "),
                output.status,
                stderr
            );
        }

        Ok(output)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowRun {
    workflow_name: Option<String>,
    conclusion: Option<String>,
    status: String,
}
