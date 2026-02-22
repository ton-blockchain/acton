use acton_config::color::OwoColorize;
use include_dir::{Dir, include_dir};
use std::fs;
use std::path::Path;

pub static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");

pub fn ensure_latest(project_root: &Path) -> anyhow::Result<()> {
    // Only run if we are in an Acton project
    if !project_root.join("Acton.toml").exists() {
        return Ok(());
    }

    let acton_dir = project_root.join(".acton");
    if !acton_dir.exists() {
        fs::create_dir_all(&acton_dir)?;
    }

    let version_path = acton_dir.join(".version");
    let current_version = env!("CARGO_PKG_VERSION");

    let needs_update = if version_path.exists() {
        let stored_version = fs::read_to_string(&version_path)?;
        stored_version.trim() != current_version
    } else {
        true
    };

    if needs_update {
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

        let tolk_stdlib_dir = acton_dir.join("tolk-stdlib");
        if !tolk_stdlib_dir.exists() {
            fs::create_dir_all(&tolk_stdlib_dir)?;
        }

        LIB_DIR.extract(&acton_dir)?;
        tolkc::compiler::TOLK_STDLIB_DIR.extract(&tolk_stdlib_dir)?;
        fs::write(version_path, current_version)?;
    }

    Ok(())
}
