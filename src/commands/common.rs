use crate::config::{ActonConfig, global_wallets_path};
use anyhow::{Context, anyhow};
use inquire::Select;
use std::path::Path;

pub mod error_fmt {
    use crate::config::ActonConfig;
    use owo_colors::OwoColorize;

    pub fn contract_not_found(config: &ActonConfig, name: &str) -> String {
        let available = available_contracts(config);
        format!(
            "Contract {} not found in Acton.toml\nAvailable contracts:\n{}",
            name.yellow(),
            available
        )
    }

    pub fn available_contracts(config: &ActonConfig) -> String {
        let contracts = config.contracts();
        if contracts.is_none() || contracts.as_ref().map(|c| c.is_empty()).unwrap_or(false) {
            return "no contracts defined yet".to_string();
        }
        contracts
            .map(|contracts| {
                contracts
                    .keys()
                    .map(|s| format!(" {}", s.yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "none".to_string())
    }

    pub fn wallet_not_found(config: &ActonConfig, name: &str) -> String {
        let wallets = config.wallets();
        if wallets.is_none() || wallets.as_ref().map(|c| c.is_empty()).unwrap_or(false) {
            return "no wallets defined yet".to_string();
        }
        let available = wallets
            .map(|contracts| {
                contracts
                    .keys()
                    .map(|s| format!(" {}", s.yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "none".to_string());
        format!(
            "Wallet {} not found in Acton.toml\nAvailable wallets:\n{}",
            name.yellow(),
            available
        )
    }

    pub fn file_not_found(path: &str) -> String {
        if path.is_empty() {
            return "Empty file path is not allowed".to_string();
        }
        let cwd = std::env::current_dir().unwrap_or(".".into());
        format!(
            "Cannot find file or directory {}",
            format!("{}/{path}", cwd.display()).yellow(),
        )
    }

    pub fn invalid_address(addr: &str) -> String {
        let hint = if (addr.starts_with("U") || addr.starts_with("E") || addr.starts_with("k"))
            && addr.len() == 47
        {
            "Did you miss the last symbol of the address (expected length is 48 but address length is 47)? "
        } else {
            ""
        };

        color_print::cformat!(
            "Address <yellow>{addr}</> is not a valid address. {hint}Enter valid address in user-friendly <green>EQ...</> or raw format <green>0:abcd...</>"
        )
    }

    pub fn script_not_found(config: &ActonConfig, name: &str) -> String {
        let available = available_scripts(config);
        format!(
            "Script {} not found in Acton.toml\nAvailable scripts:\n{}",
            name.yellow(),
            available
        )
    }

    pub fn available_scripts(config: &ActonConfig) -> String {
        let scripts = match &config.scripts {
            Some(scripts) => scripts,
            None => return "no scripts defined".to_string(),
        };

        if scripts.is_empty() {
            return "no scripts defined".to_string();
        }

        scripts
            .keys()
            .map(|s| format!(" {}", s.yellow()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn no_scripts_section() -> String {
        format!(
            "No {} section found in Acton.toml.\nTo add a script add the following section to Acton.toml:\n\n{}\n{}\n{}\n\nSee https://i582.github.io/acton/docs/commands/run/ for more information",
            "[scripts]".yellow(),
            "[scripts]".green(),
            "deploy = \"acton script scripts/deploy.tolk --broadcast\"".green(),
            "test = \"acton test tests/unit\"".green()
        )
    }
}

pub fn select_contract(
    contract_id: Option<String>,
    config: &ActonConfig,
) -> anyhow::Result<String> {
    let contract_key = match contract_id {
        Some(id) => id,
        None => {
            let contracts = config.contracts().ok_or_else(|| {
                anyhow!(
                    "No contracts configured in Acton.toml. Please add a contract configuration."
                )
            })?;

            let contract_keys: Vec<&String> = contracts.keys().collect();
            match contract_keys.len() {
                0 => anyhow::bail!(
                    "No contracts configured in Acton.toml. Please add a contract configuration."
                ),
                1 => contract_keys[0].clone(),
                _ => {
                    let contract_key = Select::new(
                        "Multiple contracts found. Please select which contract to verify:",
                        contract_keys,
                    )
                    .prompt()
                    .context("Failed to select contract")?;
                    contract_key.clone()
                }
            }
        }
    };
    Ok(contract_key)
}

pub fn select_wallet(wallet_name: Option<String>, config: &ActonConfig) -> anyhow::Result<String> {
    let wallet_name = match wallet_name {
        Some(name) => name,
        None => {
            let wallets_config = config.wallets().ok_or_else(|| {
                anyhow!("No wallets configured in Acton.toml. Please add a wallet configuration.")
            })?;

            let wallet_names: Vec<&String> = wallets_config.keys().collect();
            match wallet_names.len() {
                0 => anyhow::bail!(
                    "No wallets configured in Acton.toml. Please add a wallet configuration."
                ),
                1 => wallet_names[0].clone(),
                _ => {
                    let wallet_name = Select::new(
                        "Multiple wallets configured. Please select which wallet to use:",
                        wallet_names,
                    )
                    .prompt()
                    .context("Failed to select wallet")?;
                    wallet_name.clone()
                }
            }
        }
    };
    Ok(wallet_name)
}

pub fn create_symlink(original: &Path, link: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    std::os::unix::fs::symlink(original, link).context("Failed to create symlink")?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(original, link).context("Failed to create symlink")?;
    Ok(())
}

pub fn symlink_global_wallets() -> anyhow::Result<()> {
    if let Some(global_path) = global_wallets_path()
        && global_path.exists()
    {
        let symlink_path = Path::new("global.wallets.toml");
        if !symlink_path.exists() {
            create_symlink(&global_path, symlink_path)?;
        }
    }
    Ok(())
}
