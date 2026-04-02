use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Default)]
pub(crate) struct Git;

impl Git {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn current_branch(&self) -> Result<String> {
        self.output(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    pub(crate) fn head_commit(&self) -> Result<String> {
        self.output(&["rev-parse", "HEAD"])
    }

    pub(crate) fn has_uncommitted_changes(&self) -> Result<bool> {
        Ok(!self
            .output(&["status", "--porcelain", "--untracked-files=no"])?
            .is_empty())
    }

    pub(crate) fn commit_count_between(&self, left: &str, right: &str) -> Result<(usize, usize)> {
        let counts = self.output(&[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{left}...{right}"),
        ])?;

        let mut parts = counts.split_whitespace();
        let left = parts
            .next()
            .context("missing left rev-list count")?
            .parse::<usize>()
            .context("failed to parse left rev-list count")?;
        let right = parts
            .next()
            .context("missing right rev-list count")?
            .parse::<usize>()
            .context("failed to parse right rev-list count")?;

        if parts.next().is_some() {
            bail!("unexpected extra rev-list count output: `{counts}`");
        }

        Ok((left, right))
    }

    pub(crate) fn remote_tag_exists(&self, remote: &str, name: &str) -> Result<bool> {
        let tag_ref = format!("refs/tags/{name}");

        Ok(!self
            .output(&["ls-remote", "--tags", remote, &tag_ref])?
            .is_empty())
    }

    pub(crate) fn local_tag_exists(&self, name: &str) -> Result<bool> {
        Ok(!self.output(&["tag", "--list", name])?.is_empty())
    }

    pub(crate) fn fetch_branch(&self, remote: &str, branch: &str) -> Result<()> {
        self.output(&["fetch", remote, branch]).map(|_| ())
    }

    pub(crate) fn add_files(&self, paths: &[&str]) -> Result<()> {
        let mut args = Vec::with_capacity(paths.len() + 2);
        args.push("add");
        args.push("--");
        args.extend_from_slice(paths);

        self.output(&args).map(|_| ())
    }

    pub(crate) fn commit(&self, message: &str, args: &[&str]) -> Result<()> {
        let mut command_args = Vec::with_capacity(3 + args.len());
        command_args.push("commit");
        command_args.extend_from_slice(args);
        command_args.push("-m");
        command_args.push(message);

        self.output(&command_args).map(|_| ())
    }

    pub(crate) fn show_commit_numstat(&self, rev: &str) -> Result<String> {
        self.output(&["show", "--numstat", rev])
    }

    pub(crate) fn tag(&self, name: &str) -> Result<()> {
        self.output(&["tag", name]).map(|_| ())
    }

    pub(crate) fn delete_tag(&self, name: &str) -> Result<()> {
        self.output(&["tag", "--delete", name]).map(|_| ())
    }

    pub(crate) fn push_refs(&self, remote: &str, refs: &[&str]) -> Result<()> {
        let mut args = Vec::with_capacity(2 + refs.len());
        args.push("push");
        args.push(remote);
        args.extend_from_slice(refs);

        self.output(&args).map(|_| ())
    }

    pub(crate) fn delete_remote_tag(&self, remote: &str, name: &str) -> Result<()> {
        self.output(&["push", remote, "--delete", name]).map(|_| ())
    }

    fn output(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .output()
            .with_context(|| format!("failed to run git {}", args.join(" ")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

            bail!(
                "git {} failed with status {}: {}",
                args.join(" "),
                output.status,
                stderr
            );
        }

        String::from_utf8(output.stdout)
            .context("git output is not valid UTF-8")
            .map(|stdout| stdout.trim().to_owned())
    }
}
