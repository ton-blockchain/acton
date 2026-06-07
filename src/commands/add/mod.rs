use crate::commands::new::template as template_scaffold;
use crate::commands::new::{ProjectTemplate, template::ProjectLayout};
use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, ContractConfig, ContractDependency, ContractsConfig, default_project_mappings,
    manifest_path, project_root,
};
use anyhow::{Context, anyhow, bail};
use clap::{Subcommand, ValueEnum};
use inquire::{Confirm, Select};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{IsTerminal, stdin, stdout};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AddContractSource {
    Template,
}

#[derive(Subcommand)]
pub enum AddCommand {
    #[command(
        about = "Add a contract to the current project",
        long_about = "Add a contract and its supporting template files to the current Acton project."
    )]
    Contract {
        #[arg(
            long = "from",
            value_enum,
            value_name = "SOURCE",
            help = "Source to add the contract from"
        )]
        from: AddContractSource,
        #[arg(value_enum, value_name = "TEMPLATE", help = "Template to add")]
        template: Option<ProjectTemplate>,
        #[arg(
            long,
            help = "Overwrite existing files whose paths collide with the template"
        )]
        overwrite: bool,
    },
}

pub fn add_cmd(command: AddCommand) -> anyhow::Result<()> {
    match command {
        AddCommand::Contract {
            from: AddContractSource::Template,
            template,
            overwrite,
        } => add_contract_from_template(template, overwrite),
    }
}

fn add_contract_from_template(
    template: Option<ProjectTemplate>,
    overwrite: bool,
) -> anyhow::Result<()> {
    let interactive = stdin().is_terminal() && stdout().is_terminal();
    let selected_template = resolve_template(template, interactive)?;
    let scaffold = template_scaffold::project_scaffold(selected_template, false)
        .ok_or_else(|| anyhow!("Template '{}' is not available", selected_template.as_str()))?;
    let namespace = selected_template.as_str();
    let mut config = ActonConfig::load_manifest().context("Failed to load Acton.toml")?;
    let target_layout = detect_project_layout(&config);
    let project_root = project_root().to_path_buf();

    ensure_contracts_can_be_added(&config, scaffold.contracts())?;
    let target_files =
        template_scaffold::contract_scaffold_file_paths(scaffold, target_layout, namespace);
    confirm_or_reject_colliding_files(&project_root, &target_files, overwrite, interactive)?;

    let author = get_git_user_name().unwrap_or_else(|| "Acton User".to_owned());
    template_scaffold::create_contract_files_from_scaffold(
        scaffold,
        &project_root,
        target_layout,
        namespace,
        &author,
    )
    .with_context(|| {
        format!(
            "Failed to copy '{}' template files into the current project",
            selected_template.as_str()
        )
    })?;

    add_contracts_to_config(&mut config, scaffold.contracts(), target_layout, namespace);
    ensure_layout_mappings(&mut config, target_layout);
    save_manifest_config(&config)?;

    print_summary(selected_template, scaffold.contracts());
    Ok(())
}

fn resolve_template(
    template: Option<ProjectTemplate>,
    interactive: bool,
) -> anyhow::Result<ProjectTemplate> {
    if let Some(template) = template {
        return Ok(template);
    }

    if !interactive {
        bail!("Template is required in non-interactive mode");
    }

    let templates = template_scaffold::get_available_templates()
        .into_iter()
        .map(TemplateSelectItem)
        .collect::<Vec<_>>();

    Select::new("Select a contract template to add", templates)
        .prompt()
        .map(|item| item.0)
        .map_err(|err| anyhow!("Template selection was cancelled: {err}"))
}

fn detect_project_layout(config: &ActonConfig) -> ProjectLayout {
    let Some(mappings) = config.mappings.as_ref() else {
        return ProjectLayout::Standard;
    };

    let is_app_layout = [
        ("contracts", ProjectLayout::App.contracts_mapping()),
        ("tests", ProjectLayout::App.tests_mapping()),
        ("wrappers", ProjectLayout::App.wrappers_mapping()),
    ]
    .into_iter()
    .any(|(prefix, target)| mappings.get(prefix).is_some_and(|value| value == target));

    if is_app_layout {
        ProjectLayout::App
    } else {
        ProjectLayout::Standard
    }
}

fn confirm_or_reject_colliding_files(
    project_root: &Path,
    target_files: &[PathBuf],
    overwrite: bool,
    interactive: bool,
) -> anyhow::Result<()> {
    let collisions = target_files
        .iter()
        .filter(|relative| project_root.join(relative).exists())
        .collect::<Vec<_>>();

    if collisions.is_empty() || overwrite {
        return Ok(());
    }

    let formatted = format_colliding_files(&collisions);
    if !interactive {
        bail!("Template files already exist:\n{formatted}\nUse --overwrite to replace them.");
    }

    let overwrite = Confirm::new(&format!(
        "The template would overwrite existing files:\n{formatted}\nOverwrite these files?"
    ))
    .with_default(false)
    .prompt()
    .map_err(|err| anyhow!("Template copy was cancelled: {err}"))?;

    if overwrite {
        Ok(())
    } else {
        bail!("Template copy was cancelled to avoid overwriting existing files")
    }
}

fn format_colliding_files(collisions: &[&PathBuf]) -> String {
    collisions
        .iter()
        .map(|path| format!("  - {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_contracts_can_be_added(
    config: &ActonConfig,
    contracts: &[template_scaffold::ContractTemplate],
) -> anyhow::Result<()> {
    let Some(existing) = config.contracts.as_ref() else {
        return Ok(());
    };

    let collisions = contracts
        .iter()
        .filter(|contract| existing.contracts.contains_key(contract.id))
        .map(|contract| contract.id)
        .collect::<Vec<_>>();

    if collisions.is_empty() {
        return Ok(());
    }

    let formatted_contracts = collisions
        .iter()
        .map(|contract| contract.yellow().bold().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let (subject, verb) = if collisions.len() == 1 {
        ("Contract id", "already exists")
    } else {
        ("Contract ids", "already exist")
    };

    bail!("{subject} {formatted_contracts} {verb} in Acton.toml")
}

fn add_contracts_to_config(
    config: &mut ActonConfig,
    contracts: &[template_scaffold::ContractTemplate],
    target_layout: ProjectLayout,
    namespace: &str,
) {
    let contracts_config = config.contracts.get_or_insert_with(|| ContractsConfig {
        contracts: BTreeMap::new(),
    });

    for contract in contracts {
        let namespaced_src = template_scaffold::namespaced_scaffold_path(contract.src, namespace);
        contracts_config.contracts.insert(
            contract.id.to_owned(),
            ContractConfig {
                name: Some(contract.name.to_owned()),
                src: target_layout.remap_path(&namespaced_src),
                types: None,
                depends: Some(
                    contract
                        .depends
                        .iter()
                        .map(|dependency| ContractDependency::Simple((*dependency).to_owned()))
                        .collect(),
                ),
                output: None,
            },
        );
    }
}

fn ensure_layout_mappings(config: &mut ActonConfig, target_layout: ProjectLayout) {
    match target_layout {
        ProjectLayout::Standard => {
            config.ensure_default_mappings();
        }
        ProjectLayout::App => {
            let mappings = config.mappings.get_or_insert_with(default_project_mappings);
            mappings
                .entry("contracts".to_owned())
                .or_insert_with(|| ProjectLayout::App.contracts_mapping().to_owned());
            mappings
                .entry("tests".to_owned())
                .or_insert_with(|| ProjectLayout::App.tests_mapping().to_owned());
            mappings
                .entry("wrappers".to_owned())
                .or_insert_with(|| ProjectLayout::App.wrappers_mapping().to_owned());
        }
    }
}

fn save_manifest_config(config: &ActonConfig) -> anyhow::Result<()> {
    let contents = toml::to_string_pretty(config).context("Failed to serialize Acton.toml")?;
    fs::write(manifest_path(), contents).with_context(|| {
        format!(
            "Failed to write Acton.toml at {}",
            manifest_path().display()
        )
    })
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
                    .filter(|name| !name.is_empty())
            } else {
                None
            }
        })
}

fn print_summary(template: ProjectTemplate, contracts: &[template_scaffold::ContractTemplate]) {
    let contract_names = contracts
        .iter()
        .map(|contract| contract.id.cyan().bold().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    println!("{}", "✓ Added contract template".green().bold());
    println!(
        "  {} {}",
        "Template:".bright_black(),
        template.as_str().cyan().bold()
    );
    println!("  {} {}", "Contracts:".bright_black(), contract_names);
    println!(
        "  {} {}",
        "Updated:".bright_black(),
        manifest_path().display().to_string().cyan()
    );
}

struct TemplateSelectItem(ProjectTemplate);

impl fmt::Display for TemplateSelectItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.0.as_str(), self.0.description())
    }
}
