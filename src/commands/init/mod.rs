use crate::config::ActonConfig;
use include_dir::{Dir, include_dir};
use owo_colors::OwoColorize;

static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");

pub fn init_cmd() -> anyhow::Result<()> {
    if std::path::Path::new("Acton.toml").exists() {
        println!("{}", "Acton.toml already exists!".yellow());
        return Ok(());
    }

    let config = ActonConfig::default();
    config.save()?;

    LIB_DIR.extract(".acton")?;

    println!("{}", "✓ Initialized new Acton project".green().bold());
    println!("Created Acton.toml with default configuration");
    println!("Created .acton/ directory with standard library");

    Ok(())
}
