use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use tempfile::TempDir;

const CONVERT_FUNC_TO_TOLK_NPM_PACKAGE: &str = "@ton/convert-func-to-tolk@1.0.0";

pub fn func2tolk_cmd(
    path: String,
    output: Option<String>,
    warnings_as_comments: bool,
    no_camel_case: bool,
) -> Result<()> {
    let npm_cache_dir =
        TempDir::new().context("Failed to create a temporary npm cache directory")?;
    let mut cmd = Command::new("npx");
    cmd.env("npm_config_cache", npm_cache_dir.path())
        .env("npm_config_update_notifier", "false")
        .arg("--yes")
        .arg(CONVERT_FUNC_TO_TOLK_NPM_PACKAGE);
    if warnings_as_comments {
        cmd.arg("--warnings-as-comments");
    }
    if no_camel_case {
        cmd.arg("--no-camel-case");
    }
    if let Some(output) = output {
        cmd.arg("--output").arg(output);
    }
    cmd.arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().with_context(|| {
        "Failed to execute `npx @ton/convert-func-to-tolk@1.0.0`. Ensure Node.js/npm is installed and `npx` is available in PATH."
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        anyhow::bail!("`npx @ton/convert-func-to-tolk@1.0.0` failed with exit code {code}");
    }

    Ok(())
}
