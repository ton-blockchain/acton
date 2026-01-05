use crate::commands::common::{symlink_global_libraries, symlink_global_wallets};
use crate::config::{ActonConfig, ContractConfig, ContractsConfig, TestSettings};
use crate::stdlib;
use inquire::{Select, Text};
use owo_colors::OwoColorize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

mod licenses;
mod template;

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

# Acton related
.env
*.mnemonic
wallets.toml
global.wallets.toml

# coverage
lcov.info
gen/
";

const BASE_DOT_ENV: &str = "
# Acton uses Toncenter to access blockchain data and send messages.
# Since there's a 1 RPS limit in key-less mode, some operations require additional waiting to avoid
# exceeding the limit. We recommend obtaining a key to speed up your transactions in Acton.
# You can obtain a key in the bot at https://t.me/toncenter.
# TONCENTER_API_KEY=\"your-key-here\"
";

pub fn new_cmd(
    path: &str,
    name: Option<String>,
    description: Option<String>,
    template: Option<String>,
    license: Option<String>,
) -> anyhow::Result<()> {
    let project_path = if path == "." {
        std::env::current_dir()?
    } else {
        let path = Path::new(path).to_path_buf();
        if path.exists() {
            anyhow::bail!(color_print::cformat!(
                "Directory <yellow>{}</> is already exists, if you want to create a new project inside this directory run following commands:\n  <bold>cd {}</>\n  <bold>acton new .</>",
                path.display(),
                path.display()
            ))
        }
        path
    };

    if !project_path.exists() {
        fs::create_dir_all(&project_path)?;
    }

    let default_name = project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-acton-project");

    let project_name = if let Some(name) = name {
        name
    } else {
        Text::new("Project name:")
            .with_placeholder(default_name)
            .with_default(default_name)
            .prompt()?
    };

    let description = if let Some(description) = description {
        description
    } else {
        Text::new("Description:")
            .with_placeholder("A TON blockchain project")
            .with_default("A TON blockchain project")
            .prompt()?
    };

    let template_options = template::get_available_templates();
    let template = if let Some(template) = template {
        template
    } else {
        Select::new("Template:", template_options)
            .with_starting_cursor(0)
            .prompt()?
            .to_string()
    };

    let license_options = vec![
        "MIT",
        "Apache-2.0",
        "GPL-3.0",
        "BSD-3-Clause",
        "ISC",
        "Unlicense",
        "Other",
    ];

    let license = if let Some(license) = license {
        license
    } else {
        let license_selection = Select::new("License:", license_options)
            .with_starting_cursor(0)
            .prompt()?;

        if license_selection == "Other" {
            Text::new("Enter license:")
                .with_placeholder("MIT")
                .prompt()?
        } else {
            license_selection.to_string()
        }
    };

    let mut config = ActonConfig::default();
    config.package.name = project_name.clone();
    config.package.description = description.clone();
    config.package.license = Some(license.clone());

    std::env::set_current_dir(&project_path)?;

    // use `.` since we explicitly change current dir to project dir
    template::create_project_from_template(&template, Path::new("."))?;

    let mut contracts = BTreeMap::new();
    if template == "empty" {
        contracts.insert(
            "empty".to_owned(),
            ContractConfig {
                name: "Empty".to_owned(),
                src: "contracts/contract.tolk".to_owned(),
                depends: Some(vec![]),
                output: None,
            },
        );
    } else if template == "counter" {
        contracts.insert(
            "counter".to_owned(),
            ContractConfig {
                name: "counter".to_owned(),
                src: "contracts/counter.tolk".to_owned(),
                depends: Some(vec![]),
                output: None,
            },
        );
    } else if template == "jetton" {
        contracts.insert(
            "jetton_minter".to_owned(),
            ContractConfig {
                name: "Minter".to_owned(),
                src: "contracts/jetton-minter-contract.tolk".to_owned(),
                depends: Some(vec![]),
                output: None,
            },
        );
        contracts.insert(
            "jetton_wallet".to_owned(),
            ContractConfig {
                name: "Wallet".to_owned(),
                src: "contracts/jetton-wallet-contract.tolk".to_owned(),
                depends: Some(vec![]),
                output: None,
            },
        );
    }

    config.contracts = Some(ContractsConfig { contracts });

    config.test = Some(TestSettings {
        ..Default::default()
    });

    let mut scripts = BTreeMap::new();
    scripts.insert(
        "deploy-emulation".to_owned(),
        "acton script scripts/deploy.tolk".to_owned(),
    );
    scripts.insert(
        "deploy-testnet".to_owned(),
        "acton script scripts/deploy.tolk --broadcast --net testnet".to_owned(),
    );
    config.scripts = Some(scripts);

    config.save()?;

    stdlib::ensure_latest(Path::new("."))?;

    let author = get_git_user_name().unwrap_or_else(|| "Acton User".to_string());
    let year = chrono::Local::now().format("%Y").to_string();
    if let Some(license_text) = licenses::get_license_text(&license, &year, &author) {
        fs::write("LICENSE", license_text)?;
    }

    fs::write(".gitignore", BASE_GITIGNORE.trim_start())?;
    fs::write(".env", BASE_DOT_ENV.trim_start())?;

    if let Err(e) = symlink_global_wallets() {
        println!(
            "  {} Failed to symlink global wallets: {}",
            "Warning:".yellow().bold(),
            e
        );
    }

    if let Err(e) = symlink_global_libraries() {
        println!(
            "  {} Failed to symlink global libraries: {}",
            "Warning:".yellow().bold(),
            e
        );
    }

    if is_git_available() {
        initialize_git_repository()?;
    } else {
        println!(
            "  {} git command not found, skipping git repository initialization",
            "Warning:".yellow().bold(),
        );
    }

    println!("{}", "✓ Created new Acton project".green().bold());
    println!(
        "  {} {}",
        "Project name:".bright_black(),
        project_name.cyan().bold()
    );
    println!("  {} {}", "Description:".bright_black(), description);
    println!("  {} {}", "Template:".bright_black(), template.cyan());
    println!("  {} {}", "License:".bright_black(), license.cyan());
    println!();
    println!("Created {} with project configuration", "Acton.toml".cyan());
    println!();
    println!("Next steps:");
    println!();
    println!("  {}", "# Navigate to project".dimmed());
    println!("  {} {}", "cd".bold(), project_path.display());
    println!("  {}", "# Build your contract".dimmed());
    println!("  {} build", "acton".bold());
    println!("  {}", "# Run tests".dimmed());
    println!("  {} test", "acton".bold());

    Ok(())
}

fn get_git_user_name() -> Option<String> {
    std::process::Command::new("git")
        .args(["config", "--get", "user.name"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

fn is_git_available() -> bool {
    std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn initialize_git_repository() -> anyhow::Result<()> {
    print!("{} ", ">".green().bold());
    std::io::stdout().flush()?;
    std::process::Command::new("git")
        .arg("init")
        .status()?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize git repository"))?;

    std::process::Command::new("git")
        .args(["add", "."])
        .status()?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Failed to add files to git repository"))?;

    Ok(())
}
