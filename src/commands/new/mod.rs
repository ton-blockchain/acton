use crate::commands::common::{symlink_global_libraries, symlink_global_wallets};
use crate::commands::hooks::scaffold_and_install_default_hooks;
use crate::stdlib;
use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, ContractConfig, ContractsConfig, default_project_mappings,
};
use anyhow::{Context, anyhow};
use inquire::{Confirm, Select, Text};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeMap;
use std::fs;
use std::io::{IsTerminal, Write, stdin, stdout};
use std::path::Path;

mod licenses;
mod template;
use template::ProjectLayout;
pub use template::ProjectTemplate;

const DEFAULT_PROJECT_DESCRIPTION: &str = "A TON blockchain project";
const DEFAULT_PROJECT_LICENSE: &str = "MIT";
const BASE_GITIGNORE: &str = "
# Acton main directory
.acton/

# Build directory
build/
dist/

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
global.libraries.toml

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

const BASE_EDITORCONFIG: &str = "
root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 2
insert_final_newline = true
trim_trailing_whitespace = true

[*.tolk]
indent_size = 4
max_line_length = 100
";

const ACTON_TOML_REFERENCE_FOOTER: &str = "
# Check full Acton.toml reference and all available keys:
# https://ton-blockchain.github.io/acton/docs/acton-toml
";

#[derive(Clone, Copy)]
struct TemplateSelectItem(ProjectTemplate);

impl std::fmt::Display for TemplateSelectItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<8} {}", self.0.as_str(), self.0.description(),)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn new_cmd(
    path: &str,
    name: Option<String>,
    description: Option<String>,
    template: Option<ProjectTemplate>,
    license: Option<String>,
    app: bool,
    hooks: bool,
    agents: bool,
) -> anyhow::Result<()> {
    let project_path = if path == "." {
        std::env::current_dir()?
    } else {
        let path = Path::new(path).to_path_buf();
        if path.exists() {
            anyhow::bail!(
                "Directory {} already exists, if you want to create a new project inside this directory run following commands:\n  {}\n  {}",
                path.display().to_string().yellow(),
                format!("cd {}", path.display()).bold(),
                "acton new .".bold()
            )
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
    let interactive = stdin().is_terminal() && stdout().is_terminal();

    let project_name = if let Some(name) = name {
        name
    } else {
        Text::new("Project name:")
            .with_placeholder(default_name)
            .with_default(default_name)
            .prompt()?
    };

    let template = if let Some(template) = template {
        template
    } else {
        let template_options = template::get_available_templates()
            .into_iter()
            .map(TemplateSelectItem)
            .collect::<Vec<_>>();

        Select::new("Template:", template_options)
            .with_starting_cursor(0)
            .prompt()?
            .0
    };

    let git_available = is_git_available();
    let include_app = resolve_include_app(template, app, interactive)?;
    let configure_advanced = resolve_configure_advanced_options(
        interactive,
        git_available,
        description.is_none(),
        license.is_none(),
        hooks,
        agents,
    )?;
    let description = resolve_description(description, configure_advanced)?;
    let license = resolve_license(license, configure_advanced)?;
    let include_hooks = resolve_include_hooks(hooks, git_available, configure_advanced)?;
    let include_agents = resolve_include_agents(agents, configure_advanced)?;
    let scaffold = template::project_scaffold(template, include_app).ok_or_else(|| {
        anyhow!(
            "Template {} does not include a TypeScript app scaffold",
            template.to_string().cyan()
        )
    })?;

    let mut config = ActonConfig::default();
    config.package.name = project_name.clone();
    config.package.description = description.clone();
    config.package.license = Some(license.clone());

    std::env::set_current_dir(&project_path)?;

    // use `.` since we explicitly change current dir to project dir
    template::create_project_from_scaffold(scaffold, Path::new("."), include_agents)?;

    let mut contracts = BTreeMap::new();
    for contract in scaffold.contracts() {
        contracts.insert(
            contract.id.to_owned(),
            ContractConfig {
                name: Some(contract.name.to_owned()),
                src: contract.src.to_owned(),
                depends: Some(vec![]),
                output: None,
            },
        );
    }

    config.contracts = Some(ContractsConfig { contracts });

    config.test = Some(Default::default());

    let mut scripts = BTreeMap::new();
    scripts.insert(
        "deploy-emulation".to_owned(),
        format!("acton script {}", scaffold.deploy_script_path()),
    );
    scripts.insert(
        "deploy-testnet".to_owned(),
        format!(
            "acton script {} --net testnet",
            scaffold.deploy_script_path()
        ),
    );
    config.scripts = Some(scripts);
    config.mappings = Some(project_mappings(scaffold.layout()));

    let mut acton_toml = toml::to_string_pretty(&config)?;
    acton_toml.push_str(ACTON_TOML_REFERENCE_FOOTER);
    fs::write("Acton.toml", acton_toml)?;

    if scaffold.layout().includes_typescript_app() {
        update_npm_package_metadata(&project_name)?;
    }

    stdlib::ensure_latest(Path::new("."))?;

    let author = get_git_user_name().unwrap_or_else(|| "Acton User".to_string());
    let year = chrono::Local::now().format("%Y").to_string();
    if let Some(license_text) = licenses::get_license_text(&license, &year, &author) {
        fs::write("LICENSE", license_text)?;
    }

    fs::write(".gitignore", BASE_GITIGNORE.trim_start())?;
    fs::write(".env", BASE_DOT_ENV.trim_start())?;
    fs::write(".editorconfig", BASE_EDITORCONFIG.trim_start())?;

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

    if git_available {
        initialize_git_repository()?;
        if include_hooks {
            scaffold_and_install_default_hooks(Path::new("."))?;
        }
        stage_git_repository()?;
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
    println!(
        "  {} {}",
        "Template:".bright_black(),
        template.to_string().cyan()
    );
    if scaffold.layout().includes_typescript_app() {
        println!(
            "  {} {}",
            "TypeScript app:".bright_black(),
            "included".cyan()
        );
    }
    if include_hooks {
        println!("  {} {}", "Git hooks:".bright_black(), "installed".cyan());
    }
    if include_agents {
        println!("  {} {}", "AGENTS.md:".bright_black(), "included".cyan());
    }
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
    if scaffold.layout().includes_typescript_app() {
        println!("  {}", "# Install app dependencies".dimmed());
        println!("  npm ci");
        println!("  {}", "# Start the TypeScript app".dimmed());
        println!("  npm run dev");
    }

    Ok(())
}

fn resolve_include_app(
    template: ProjectTemplate,
    app: bool,
    interactive: bool,
) -> anyhow::Result<bool> {
    if app {
        if !template::template_supports_app(template) {
            anyhow::bail!(
                "Template {} does not include a TypeScript dApp",
                template.to_string().cyan()
            );
        }

        return Ok(true);
    }

    if !template::template_supports_app(template) {
        return Ok(false);
    }

    if interactive {
        Confirm::new("Include the TypeScript dApp?")
            .with_default(false)
            .prompt()
            .map_err(Into::into)
    } else {
        Ok(false)
    }
}

fn resolve_configure_advanced_options(
    interactive: bool,
    git_available: bool,
    missing_description: bool,
    missing_license: bool,
    hooks: bool,
    agents: bool,
) -> anyhow::Result<bool> {
    if !interactive {
        return Ok(false);
    }

    let has_optional_advanced_prompts =
        missing_description || missing_license || (git_available && !hooks) || !agents;
    if !has_optional_advanced_prompts {
        return Ok(false);
    }

    Confirm::new("Do you want to configure advanced options (Git hooks, license, etc.)?")
        .with_default(false)
        .prompt()
        .map_err(Into::into)
}

fn resolve_description(
    description: Option<String>,
    configure_advanced: bool,
) -> anyhow::Result<String> {
    if let Some(description) = description {
        return Ok(description);
    }

    if !configure_advanced {
        return Ok(DEFAULT_PROJECT_DESCRIPTION.to_owned());
    }

    Text::new("Description:")
        .with_placeholder(DEFAULT_PROJECT_DESCRIPTION)
        .with_default(DEFAULT_PROJECT_DESCRIPTION)
        .prompt()
        .map_err(Into::into)
}

fn resolve_license(license: Option<String>, configure_advanced: bool) -> anyhow::Result<String> {
    if let Some(license) = license {
        return Ok(license);
    }

    if !configure_advanced {
        return Ok(DEFAULT_PROJECT_LICENSE.to_owned());
    }

    let license_options = vec![
        "MIT",
        "Apache-2.0",
        "GPL-3.0",
        "BSD-3-Clause",
        "ISC",
        "Unlicense",
        "Other",
    ];
    let license_selection = Select::new("License:", license_options)
        .with_starting_cursor(0)
        .prompt()?;

    if license_selection == "Other" {
        Text::new("Enter license:")
            .with_placeholder(DEFAULT_PROJECT_LICENSE)
            .prompt()
            .map_err(Into::into)
    } else {
        Ok(license_selection.to_string())
    }
}

fn resolve_include_hooks(
    hooks: bool,
    git_available: bool,
    configure_advanced: bool,
) -> anyhow::Result<bool> {
    if hooks {
        if !git_available {
            anyhow::bail!("Git hooks require the `git` command to be available in PATH");
        }

        return Ok(true);
    }

    if !git_available {
        return Ok(false);
    }

    if configure_advanced {
        Confirm::new("Set up Git hooks to run checks before each commit?")
            .with_default(false)
            .prompt()
            .map_err(Into::into)
    } else {
        Ok(false)
    }
}

fn resolve_include_agents(agents: bool, configure_advanced: bool) -> anyhow::Result<bool> {
    if agents {
        return Ok(true);
    }

    if configure_advanced {
        Confirm::new("Include AGENTS.md guidance for coding agents?")
            .with_default(false)
            .prompt()
            .map_err(Into::into)
    } else {
        Ok(false)
    }
}

fn project_mappings(layout: ProjectLayout) -> BTreeMap<String, String> {
    if !layout.includes_typescript_app() {
        return default_project_mappings();
    }

    let mut mappings = default_project_mappings();
    mappings.insert(
        "contracts".to_owned(),
        layout.contracts_mapping().to_owned(),
    );
    mappings.insert("tests".to_owned(), layout.tests_mapping().to_owned());
    mappings.insert("wrappers".to_owned(), layout.wrappers_mapping().to_owned());
    mappings
}

fn update_npm_package_metadata(project_name: &str) -> anyhow::Result<()> {
    let normalized_name = normalize_npm_package_name(project_name);
    let has_workspaces = update_package_json_name(&normalized_name)?;
    update_package_lock_name(&normalized_name, has_workspaces)?;
    Ok(())
}

fn update_package_json_name(normalized_name: &str) -> anyhow::Result<bool> {
    let package_json_path = Path::new("package.json");

    if !package_json_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(package_json_path)
        .with_context(|| format!("Failed to read {}", package_json_path.display()))?;
    let mut package_json: JsonValue = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", package_json_path.display()))?;
    let has_workspaces = matches!(
        &package_json,
        JsonValue::Object(object) if object.contains_key("workspaces")
    );

    if matches!(package_json, JsonValue::Object(_))
        && let Some(updated_content) =
            replace_top_level_json_string_field(&content, "name", normalized_name)
    {
        fs::write(package_json_path, updated_content)
            .with_context(|| format!("Failed to write {}", package_json_path.display()))?;
        return Ok(has_workspaces);
    }

    if let JsonValue::Object(object) = &mut package_json {
        object.insert(
            "name".to_owned(),
            JsonValue::String(normalized_name.to_owned()),
        );
    } else {
        let mut object = JsonMap::new();
        object.insert(
            "name".to_owned(),
            JsonValue::String(normalized_name.to_owned()),
        );
        package_json = JsonValue::Object(object);
    }

    let formatted =
        serde_json::to_string_pretty(&package_json).context("Failed to serialize package.json")?;
    fs::write(package_json_path, format!("{formatted}\n"))
        .with_context(|| format!("Failed to write {}", package_json_path.display()))?;
    Ok(has_workspaces)
}

fn replace_top_level_json_string_field(content: &str, field: &str, value: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let field_literal = serde_json::to_string(field).ok()?;
    let replacement_literal = serde_json::to_string(value).ok()?;
    let mut index = 0;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => {
                if depth == 1 && bytes[index..].starts_with(field_literal.as_bytes()) {
                    let key_end = index + field_literal.len();
                    let mut value_start = key_end;

                    while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                        value_start += 1;
                    }

                    if bytes.get(value_start) != Some(&b':') {
                        in_string = true;
                        index += 1;
                        continue;
                    }

                    value_start += 1;
                    while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
                        value_start += 1;
                    }

                    if bytes.get(value_start) != Some(&b'"') {
                        return None;
                    }

                    let value_end = find_json_string_end(bytes, value_start)?;
                    let mut updated = String::with_capacity(
                        content.len() + replacement_literal.len() - (value_end - value_start),
                    );
                    updated.push_str(&content[..value_start]);
                    updated.push_str(&replacement_literal);
                    updated.push_str(&content[value_end..]);
                    return Some(updated);
                }

                in_string = true;
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }

        index += 1;
    }

    None
}

fn find_json_string_end(bytes: &[u8], start_quote: usize) -> Option<usize> {
    let mut index = start_quote + 1;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return Some(index + 1);
        }

        index += 1;
    }

    None
}

fn update_package_lock_name(normalized_name: &str, has_workspaces: bool) -> anyhow::Result<()> {
    let package_lock_path = Path::new("package-lock.json");

    if !package_lock_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(package_lock_path)
        .with_context(|| format!("Failed to read {}", package_lock_path.display()))?;
    let mut package_lock: JsonValue = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", package_lock_path.display()))?;

    let JsonValue::Object(root) = &mut package_lock else {
        anyhow::bail!("{} must contain a JSON object", package_lock_path.display());
    };

    root.insert(
        "name".to_owned(),
        JsonValue::String(normalized_name.to_owned()),
    );

    if let Some(packages) = root.get_mut("packages").and_then(JsonValue::as_object_mut) {
        if let Some(root_package) = packages.get_mut("").and_then(JsonValue::as_object_mut) {
            root_package.insert(
                "name".to_owned(),
                JsonValue::String(normalized_name.to_owned()),
            );
        }

        if !has_workspaces {
            packages.retain(|path, _| path.is_empty() || path.starts_with("node_modules"));
        }
    }

    let formatted =
        serde_json::to_string_pretty(&package_lock).context("Failed to serialize package-lock")?;
    fs::write(package_lock_path, format!("{formatted}\n"))
        .with_context(|| format!("Failed to write {}", package_lock_path.display()))?;
    Ok(())
}

fn normalize_npm_package_name(project_name: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_separator = false;

    for ch in project_name.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !normalized.is_empty() {
            normalized.push('-');
            last_was_separator = true;
        }
    }

    while normalized.ends_with('-') {
        normalized.pop();
    }

    if normalized.is_empty() {
        "acton-app".to_owned()
    } else {
        normalized
    }
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
    stdout().flush()?;
    std::process::Command::new("git")
        .arg("init")
        .status()?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize git repository"))?;

    Ok(())
}

fn stage_git_repository() -> anyhow::Result<()> {
    std::process::Command::new("git")
        .args(["add", "."])
        .status()?
        .success()
        .then_some(())
        .ok_or_else(|| anyhow::anyhow!("Failed to add files to git repository"))?;

    Ok(())
}
