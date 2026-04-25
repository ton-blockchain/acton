use acton_config::color::OwoColorize;
use anyhow::Context;
use include_dir::{Dir, include_dir};
use std::fs;
use std::path::Path;

static EMPTY_UI_TEMPLATE_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/src/commands/create_app/template");

const APP_DIR: &str = "app";

pub fn create_app_cmd(path: Option<&Path>) -> anyhow::Result<()> {
    let target_dir = path.unwrap_or_else(|| Path::new(APP_DIR));
    if target_dir.exists() {
        anyhow::bail!(
            "Directory {} already exists. Delete it before running `acton create-app`.",
            target_dir.display().to_string().yellow()
        );
    }

    extract_template_dir(&EMPTY_UI_TEMPLATE_DIR, target_dir)
        .context("Failed to create app scaffold")?;

    println!("{}", "✓ Created TypeScript app scaffold".green().bold());
    println!(
        "  {} {}",
        "Directory:".bright_black(),
        target_dir.display().to_string().cyan()
    );
    println!();
    println!("Next steps:");
    println!();
    println!("  {}", "# Install app dependencies".dimmed());
    println!("  {} {}", "cd".bold(), target_dir.display());
    println!("  npm ci");
    println!("  {}", "# Start the TypeScript app".dimmed());
    println!("  npm run dev");

    Ok(())
}

fn extract_template_dir(dir: &Dir<'static>, target_dir: &Path) -> std::io::Result<()> {
    for entry in dir.entries() {
        let path = target_dir.join(entry.path());

        if let Some(subdir) = entry.as_dir() {
            fs::create_dir_all(&path)?;
            extract_template_dir(subdir, target_dir)?;
            continue;
        }

        if let Some(file) = entry.as_file() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(path, file.contents())?;
        }
    }

    Ok(())
}
