use crate::build_info;
use acton_config::color::OwoColorize;
use include_dir::{Dir, include_dir};
use std::env;
use std::fs;
use std::path::Path;

pub static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");
pub const DISABLE_AUTO_STDLIB_ENV: &str = "ACTON_DISABLE_AUTO_STDLIB";

pub(crate) fn current_stdlib_version() -> String {
    let package_version = build_info::PACKAGE_VERSION;
    let release_channel = build_info::RELEASE_CHANNEL;
    let git_hash = build_info::GIT_HASH;

    if release_channel == "trunk" {
        format!("{package_version}-trunk+{git_hash}")
    } else {
        package_version.to_owned()
    }
}

pub fn ensure_latest(project_root: &Path) -> anyhow::Result<()> {
    ensure_latest_with_output(project_root, true)
}

pub(crate) fn ensure_latest_quiet(project_root: &Path) -> anyhow::Result<()> {
    ensure_latest_with_output(project_root, false)
}

fn ensure_latest_with_output(project_root: &Path, report_progress: bool) -> anyhow::Result<()> {
    if auto_install_disabled() {
        return Ok(());
    }

    // Only run if we are in an Acton project
    if !project_root.join("Acton.toml").exists() {
        return Ok(());
    }

    install_latest(project_root, false, report_progress)
}

pub fn update_latest(project_root: &Path) -> anyhow::Result<()> {
    install_latest(project_root, true, true)
}

#[must_use]
fn auto_install_disabled() -> bool {
    env::var_os(DISABLE_AUTO_STDLIB_ENV).is_some_and(|value| {
        let value = value.to_string_lossy().trim().to_ascii_lowercase();
        !value.is_empty() && value != "0" && value != "false"
    })
}

fn install_latest(project_root: &Path, force: bool, report_progress: bool) -> anyhow::Result<()> {
    let acton_dir = project_root.join(".acton");
    if !acton_dir.exists() {
        fs::create_dir_all(&acton_dir)?;
    }

    let version_path = acton_dir.join(".version");
    let current_version = current_stdlib_version();

    let needs_update = if force || !version_path.exists() {
        true
    } else {
        let stored_version = fs::read_to_string(&version_path)?;
        stored_version.trim() != current_version
    };

    if needs_update {
        if report_progress {
            if version_path.exists() {
                println!(
                    "    {} standard library to v{}",
                    "Updating".green().bold(),
                    current_version
                );
            } else {
                println!(
                    "  {} standard library v{}",
                    "Installing".green().bold(),
                    current_version
                );
            }
        }

        let tolk_stdlib_dir = acton_dir.join("tolk-stdlib");
        if !tolk_stdlib_dir.exists() {
            fs::create_dir_all(&tolk_stdlib_dir)?;
        }

        LIB_DIR.extract(&acton_dir)?;
        tolk_compiler::compiler::TOLK_STDLIB_DIR.extract(&tolk_stdlib_dir)?;
        fs::write(version_path, &current_version)?;
    }

    Ok(())
}
