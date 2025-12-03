use crate::config::ActonConfig;
use include_dir::{Dir, include_dir};
use inquire::{Select, Text};
use owo_colors::OwoColorize;
use std::fs;
use std::path::Path;

static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");
static TOLK_STDLIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/crates/tolkc/assets/tolk-stdlib");
const BASE_GITIGNORE: &str = "
# Acton main directory
.acton/

# Build directory
build/

.DS_Store
node_modules/

# VS Code
.vscode/*
.history/
*.vsix

# IDEA files
.idea

# Vim
Session.vim
.vim/

# Other private editor folders
.nvim/
.emacs/
.helix/

.env
";

pub fn new_cmd(path: &str) -> anyhow::Result<()> {
    let project_path = if path == "." {
        std::env::current_dir()?
    } else {
        Path::new(path).to_path_buf()
    };

    let acton_toml_path = project_path.join("Acton.toml");
    if acton_toml_path.exists() {
        println!(
            "{}",
            "Acton.toml already exists in the target directory!".yellow()
        );
        return Ok(());
    }

    if !project_path.exists() {
        fs::create_dir_all(&project_path)?;
    }

    let default_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-acton-project");

    let project_name = Text::new("Project name:")
        .with_placeholder(default_name)
        .with_default(default_name)
        .prompt()?;

    let description = Text::new("Description:")
        .with_placeholder("A TON blockchain project")
        .with_default("A TON blockchain project")
        .prompt()?;

    let license_options = vec![
        "MIT",
        "Apache-2.0",
        "GPL-3.0",
        "BSD-3-Clause",
        "ISC",
        "Unlicense",
        "Other",
    ];

    let license = Select::new("License:", license_options)
        .with_starting_cursor(0)
        .prompt()?;

    let license = if license == "Other" {
        Text::new("Enter license:")
            .with_placeholder("MIT")
            .prompt()?
    } else {
        license.to_string()
    };

    let mut config = ActonConfig::default();
    config.package.name = project_name.clone();
    config.package.description = description.clone();
    config.package.license = Some(license.clone());

    std::env::set_current_dir(&project_path)?;
    config.save()?;

    fs::create_dir_all(".acton/tolk-stdlib")?;
    LIB_DIR.extract(".acton")?;
    TOLK_STDLIB_DIR.extract(".acton/tolk-stdlib")?;

    fs::create_dir_all("contracts/")?;
    fs::create_dir_all("tests/")?;

    fs::write(".gitignore", BASE_GITIGNORE.trim_start())?;

    println!("{}", "✓ Created new Acton project".green().bold());
    println!(
        "  {} {}",
        "Project name:".bright_black(),
        project_name.cyan().bold()
    );
    println!("  {} {}", "Description:".bright_black(), description);
    println!("  {} {}", "License:".bright_black(), license.cyan());
    println!();
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
