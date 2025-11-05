use crate::config::ActonConfig;
use include_dir::{Dir, include_dir};
use owo_colors::OwoColorize;
use std::fs;

static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");
static TOLK_STDLIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/crates/tolkc/assets/tolk-stdlib");

pub fn init_cmd() -> anyhow::Result<()> {
    if std::path::Path::new("Acton.toml").exists() {
        println!("{}", "Acton.toml already exists!".yellow());
        return Ok(());
    }

    let config = ActonConfig::default();
    config.save()?;

    fs::create_dir_all(".acton/tolk-stdlib")?;
    LIB_DIR.extract(".acton")?;
    TOLK_STDLIB_DIR.extract(".acton/tolk-stdlib")?;

    println!("{}", "✓ Initialized new Acton project".green().bold());
    println!("Created {} with project configuration", "Acton.toml".cyan());
    println!(
        "Created {} directory with standard library",
        ".acton/".cyan()
    );
    println!(
        "Created {} directory with Tolk standard library",
        ".acton/tolk-stdlib".cyan()
    );

    Ok(())
}
