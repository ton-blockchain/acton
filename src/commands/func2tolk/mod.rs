use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use tempfile::TempDir;

const CONVERT_FUNC_TO_TOLK_NPM_PACKAGE_NAME: &str = "@ton/convert-func-to-tolk";
const DEFAULT_CONVERT_FUNC_TO_TOLK_VERSION: &str = "1.0.0";

#[inline]
#[must_use]
pub const fn default_func2tolk_version() -> &'static str {
    DEFAULT_CONVERT_FUNC_TO_TOLK_VERSION
}

fn convert_func_to_tolk_npm_package(version: &str) -> String {
    format!("{CONVERT_FUNC_TO_TOLK_NPM_PACKAGE_NAME}@{version}")
}

pub fn func2tolk_cmd(
    path: String,
    output: Option<String>,
    warnings_as_comments: bool,
    no_camel_case: bool,
    version: String,
) -> Result<()> {
    let npm_package = convert_func_to_tolk_npm_package(&version);
    let npm_cache_dir =
        TempDir::new().context("Failed to create a temporary npm cache directory")?;
    let mut cmd = Command::new("npx");
    cmd.env("npm_config_cache", npm_cache_dir.path())
        .env("npm_config_update_notifier", "false")
        .arg("--yes")
        .arg(&npm_package);
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
        format!(
            "Failed to execute `npx {npm_package}`. Ensure Node.js/npm is installed and `npx` is available in PATH."
        )
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        anyhow::bail!("`npx {npm_package}` failed with exit code {code}");
    }

    Ok(())
}
