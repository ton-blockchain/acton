use crate::commands::common::{symlink_global_libraries, symlink_global_wallets};
use crate::stdlib;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig, ContractsConfig};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;
use tree_sitter::Node;
use walkdir::WalkDir;

const GITIGNORE_GROUPS: &[(&str, &[&str])] = &[
    (
        "# Acton related files",
        &[
            ".acton/",
            "gen/",
            "build/",
            "lcov.info",
            "libraries.toml",
            "global.libraries.toml",
        ],
    ),
    (
        "# Mnemonic and wallet files",
        &[".env", "*.mnemonic", "wallets.toml", "global.wallets.toml"],
    ),
];

pub fn init_cmd() -> anyhow::Result<()> {
    let acton_toml_exists = Path::new("Acton.toml").exists();

    if acton_toml_exists {
        println!(
            "    {} Acton.toml project configuration",
            "Skipping".green().bold()
        );
        if patch_default_mappings()? {
            println!(
                "     {} Acton.toml with default mappings",
                "Patched".green().bold()
            );
        }
    } else {
        let mut config = ActonConfig::default();
        config.ensure_default_mappings();

        let discovered_contracts = discover_contracts();
        let contract_count = discovered_contracts.len();

        if discovered_contracts.is_empty() {
            println!(
                "       {} no contracts in the current directory",
                "Found".green().bold()
            );
        } else {
            println!(
                "  {} {} contract{}",
                "Discovered".bold().green(),
                contract_count,
                if contract_count == 1 { "" } else { "s" }
            );
            for (key, contract) in &discovered_contracts {
                println!("             {} ({})", contract.name.cyan(), key);
            }
            config.contracts = Some(ContractsConfig {
                contracts: discovered_contracts,
            });
        }

        config.save()?;
        println!(
            "     {} Acton.toml with project configuration",
            "Created".green().bold()
        );
    }

    stdlib::ensure_latest(Path::new("."))?;

    patch_or_create_gitignore()?;

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

    if acton_toml_exists {
        println!("\n{}", "✓ Updated Acton project".green().bold());
    } else {
        println!("\n{}", "✓ Initialized new Acton project".green().bold());
    }

    Ok(())
}

fn patch_default_mappings() -> anyhow::Result<bool> {
    let content = fs::read_to_string("Acton.toml")?;
    let mut config: ActonConfig = toml::from_str(&content)?;

    if !config.ensure_default_mappings() {
        return Ok(false);
    }

    config.save()?;
    Ok(true)
}

fn patch_or_create_gitignore() -> anyhow::Result<()> {
    let content = if fs::exists(".gitignore").unwrap_or(false) {
        fs::read_to_string(".gitignore")?
    } else {
        String::new()
    };
    let mut existing_lines = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<HashSet<_>>();

    let mut to_add = String::new();

    for (heading, patterns) in GITIGNORE_GROUPS {
        append_missing_group_to_gitignore(&mut existing_lines, &mut to_add, heading, patterns);
    }

    if !to_add.is_empty() {
        let mut new_content = content;
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&to_add);
        fs::write(".gitignore", new_content)?;
        println!(
            "     {} .gitignore with Acton patterns",
            "Patched".green().bold()
        );
    }
    Ok(())
}

fn append_missing_group_to_gitignore(
    existing_lines: &mut HashSet<String>,
    output: &mut String,
    heading: &str,
    patterns: &[&str],
) {
    let missing_patterns = patterns
        .iter()
        .copied()
        .filter(|pattern| !existing_lines.contains(*pattern))
        .collect::<Vec<_>>();

    if missing_patterns.is_empty() {
        return;
    }

    output.push('\n');
    if !existing_lines.contains(heading) {
        output.push_str(heading);
        output.push('\n');
        existing_lines.insert(heading.to_string());
    }

    for pattern in missing_patterns {
        output.push_str(pattern);
        output.push('\n');
        existing_lines.insert(pattern.to_string());
    }
}

fn discover_contracts() -> BTreeMap<String, ContractConfig> {
    let mut contracts = BTreeMap::new();

    for entry in WalkDir::new(".")
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let path = e.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !file_name.starts_with('.')
                && !matches!(file_name, "node_modules" | "target" | ".git" | ".acton")
        })
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if path.extension() != Some("tolk".as_ref()) {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let tree = match tolk_syntax::parse(&content) {
            Ok(tree) => tree,
            Err(_) => continue,
        };

        // treat all files with onInternalMessage as a contract entry file
        if !has_on_internal_message_function(&tree.root_node(), &content) {
            continue;
        }

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let relative_path = path
            .strip_prefix(".")
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let contract_key = file_stem.replace('-', "_");
        let contract_name = format_contract_name(file_stem);

        let contract_config = ContractConfig {
            name: contract_name,
            src: relative_path,
            depends: Some(vec![]),
            output: None,
        };

        contracts.insert(contract_key, contract_config);
    }

    contracts
}

fn has_on_internal_message_function(root_node: &Node<'_>, content: &str) -> bool {
    let mut cursor = root_node.walk();
    for child in root_node.children(&mut cursor) {
        if child.kind() == "function_declaration"
            && let Some(name_node) = child.child_by_field_name("name")
        {
            let name = name_node.utf8_text(content.as_bytes()).unwrap_or("");
            if name == "onInternalMessage" {
                return true;
            }
        }
    }
    false
}

fn format_contract_name(file_stem: &str) -> String {
    file_stem
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars.as_str().chars()).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
