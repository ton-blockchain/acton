use anyhow::Result;
use std::path::{Path, PathBuf};

mod linter;
mod stdlib;

const DEFAULT_STDLIB_OUT: &str = "docs/content/docs/standard_library";
const DEFAULT_LINTER_OUT: &str = "docs/content/docs/linter";
const GITHUB_SOURCE_BASE: &str = "https://github.com/i582/acton/blob/master";

pub fn docgen_cmd(output: Option<String>) -> Result<()> {
    let stdlib_output = output.unwrap_or_else(|| DEFAULT_STDLIB_OUT.to_string());
    let stdlib_out_dir = PathBuf::from(&stdlib_output);

    stdlib::generate_stdlib_docs(Path::new("lib"), &stdlib_out_dir)?;

    let linter_out_dir = stdlib_out_dir
        .parent()
        .map(|parent| parent.join("linter"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LINTER_OUT));

    linter::generate_linter_docs(&linter_out_dir)?;

    Ok(())
}
