use std::process::Command;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Clone, Copy, Default)]
pub(crate) struct Github;

impl Github {
    pub(crate) const fn new() -> Self {
        Self
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
                self.ensure_workflow_run_completed_with_conclusion(run, branch, &["success"])
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

    pub(crate) fn download_release_asset(&self, tag: &str, asset_name: &str) -> Result<Vec<u8>> {
        let output = self
            .command_output(&[
                "release",
                "download",
                tag,
                "--pattern",
                asset_name,
                "--output",
                "-",
            ])
            .with_context(|| {
                format!(
                    "failed to download asset `{asset_name}` from release `{tag}` in the current repository"
                )
            })?;

        Ok(output.stdout)
    }

    pub(crate) fn list_cache_entries(&self) -> Result<Vec<GithubCacheEntry>> {
        self.json_output(&[
            "cache",
            "list",
            "--limit",
            "1000",
            "--json",
            "id,key,sizeInBytes,createdAt,lastAccessedAt,ref",
        ])
    }

    pub(crate) fn delete_cache_entry(&self, cache_entry_id: &str) -> Result<()> {
        self.command_output(&["cache", "delete", cache_entry_id])
            .map(|_| ())
    }

    pub(crate) fn ensure_release_does_not_exist(&self, tag: &str) -> Result<()> {
        let output = Command::new("gh")
            .args(["release", "view", tag])
            .output()
            .with_context(|| format!("failed to run gh release view {tag}"))?;

        if output.status.success() {
            bail!("GitHub release `{tag}` already exists");
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        if stderr.contains("release not found")
            || stderr.contains("HTTP 404")
            || stderr.contains("Not Found")
        {
            return Ok(());
        }

        bail!(
            "gh release view {tag} failed with status {}: {}",
            output.status,
            stderr
        );
    }

    pub(crate) fn latest_release_workflow_run_for_tag(&self, tag: &str) -> Result<WorkflowRun> {
        let runs = self.json_output::<Vec<WorkflowRun>>(&[
            "run",
            "list",
            "--workflow",
            "Release",
            "--event",
            "push",
            "--limit",
            "100",
            "--json",
            "headBranch,startedAt,workflowName,status,conclusion",
        ])?;

        let Some(run) = find_latest_workflow_run_for_ref(&runs, tag) else {
            bail!("no GitHub Actions `Release` run found for tag `{tag}`");
        };

        Ok(run.clone())
    }

    pub(crate) fn ensure_workflow_run_completed_with_conclusion(
        &self,
        run: &WorkflowRun,
        ref_name: &str,
        expected_conclusions: &[&str],
    ) -> Result<()> {
        let workflow_name = run.workflow_name.as_deref().unwrap_or("<unnamed workflow>");

        if run.status != "completed" {
            bail!("workflow `{workflow_name}` for ref `{ref_name}` is not completed");
        }

        let actual_conclusion = run.conclusion.as_deref().unwrap_or("<none>");
        if !expected_conclusions.contains(&actual_conclusion) {
            let expected = expected_conclusions
                .iter()
                .map(|conclusion| format!("`{conclusion}`"))
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "workflow `{workflow_name}` for ref `{ref_name}` concluded with `{actual_conclusion}` instead of one of {expected}"
            );
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkflowRun {
    head_branch: Option<String>,
    started_at: Option<String>,
    workflow_name: Option<String>,
    conclusion: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GithubCacheEntry {
    pub(crate) id: u64,
    #[serde(rename = "ref")]
    pub(crate) branch: String,
    pub(crate) key: String,
    pub(crate) size_in_bytes: u64,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) last_accessed_at: Option<DateTime<Utc>>,
}

fn find_latest_workflow_run_for_ref<'a>(
    runs: &'a [WorkflowRun],
    ref_name: &str,
) -> Option<&'a WorkflowRun> {
    runs.iter()
        .filter(|run| run.head_branch.as_deref() == Some(ref_name))
        .filter(|run| run.started_at.is_some())
        .max_by_key(|run| run.started_at.as_deref())
}
