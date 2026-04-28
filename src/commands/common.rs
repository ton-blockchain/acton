use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, global_libraries_path, global_wallets_path,
    project_root as configured_project_root,
};
use anyhow::{Context, anyhow};
use inquire::Select;
use std::io::{IsTerminal, stdin, stdout};
use std::path::Path;
use ton_executor::ExecutorVerbosity;

pub mod error_fmt {
    use acton_config::color::OwoColorize;
    use acton_config::config::ActonConfig;
    use std::path::Path;

    #[must_use]
    pub fn contract_not_found(config: &ActonConfig, name: &str) -> String {
        let display_name_matches = contract_ids_by_display_name(config, name);
        if !display_name_matches.is_empty() {
            return contract_not_found_for_display_name(config, name, &display_name_matches);
        }

        let available = available_contracts(config);
        format!(
            "Contract {} not found in Acton.toml\nAvailable contracts:\n{}",
            name.yellow(),
            available
        )
    }

    fn contract_not_found_for_display_name(
        config: &ActonConfig,
        provided_name: &str,
        matches: &[(String, String)],
    ) -> String {
        let available = available_contracts(config);
        let examples = matches
            .iter()
            .map(|(contract_id, display_name)| {
                let header = format!("[contracts.{contract_id}]");
                let id_marker = format!(
                    "{}{} contract ID to pass to Acton",
                    " ".repeat("[contracts.".len()),
                    "^".repeat(contract_id.len()),
                );
                let display_line = format!("display-name = \"{display_name}\"");
                let display_marker = format!(
                    "{}{} display-name shown in logs/UI",
                    " ".repeat("display-name = \"".len()),
                    "^".repeat(display_name.len()),
                );
                format!("{header}\n{id_marker}\n{display_line}\n{display_marker}")
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        if matches.len() == 1 {
            let contract_id = matches[0].0.as_str();
            return format!(
                "Contract {} not found in Acton.toml\n\nIt looks like you passed the contract display-name instead of the contract ID.\n\nIn Acton.toml this contract is configured as:\n{}\n\nPass {} instead of {}.\n\nAvailable contract IDs:\n{}",
                provided_name.yellow(),
                examples,
                contract_id.green().bold(),
                provided_name.yellow(),
                available
            );
        }

        format!(
            "Contract {} not found in Acton.toml\n\nIt looks like you passed a contract display-name instead of a contract ID.\nThis display-name matches multiple contracts.\n\nIn Acton.toml use one of these contract IDs:\n{}\n\nPass one of the IDs above instead of {}.\n\nAvailable contract IDs:\n{}",
            provided_name.yellow(),
            examples,
            provided_name.yellow(),
            available
        )
    }

    fn contract_ids_by_display_name(
        config: &ActonConfig,
        display_name: &str,
    ) -> Vec<(String, String)> {
        config
            .contracts()
            .into_iter()
            .flat_map(|contracts| contracts.iter())
            .filter_map(|(contract_id, contract)| {
                let configured_display_name = contract.display_name(contract_id);
                (configured_display_name == display_name)
                    .then(|| (contract_id.clone(), configured_display_name.to_owned()))
            })
            .collect()
    }

    #[must_use]
    pub fn available_contracts(config: &ActonConfig) -> String {
        let contracts = config.contracts();
        if contracts.is_none() || contracts.as_ref().is_some_and(|c| c.is_empty()) {
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

    #[must_use]
    pub fn wallet_not_found(config: &ActonConfig, name: &str) -> String {
        let wallets = config.wallets();
        if wallets.is_none() || wallets.as_ref().is_some_and(|c| c.is_empty()) {
            return format!("Wallet {} not found. {}", name.yellow(), no_wallets_found());
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
            "Wallet {} not found in wallets.toml and global.wallets.toml\nAvailable wallets:\n{}",
            name.yellow(),
            available
        )
    }

    #[must_use]
    pub fn library_not_found(config: &ActonConfig, name: &str) -> String {
        let libraries = config.libraries();
        if libraries.is_none() || libraries.as_ref().is_some_and(|c| c.is_empty()) {
            return format!(
                "Library {} not found. {}",
                name.yellow(),
                no_libraries_found()
            );
        }
        let available = libraries
            .map(|libs| {
                libs.keys()
                    .map(|s| format!(" {}", s.yellow()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| "none".to_string());
        format!(
            "Library {} not found in libraries.toml and global.libraries.toml\nAvailable libraries:\n{}",
            name.yellow(),
            available
        )
    }

    #[must_use]
    pub fn file_not_found(path: &str) -> String {
        if path.is_empty() {
            return "Empty file path is not allowed".to_string();
        }
        let path = Path::new(path);

        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };
        format!(
            "Cannot find file or directory {}",
            absolute_path.display().to_string().yellow(),
        )
    }

    #[must_use]
    pub fn invalid_address(addr: &str) -> String {
        let hint = if (addr.starts_with('U') || addr.starts_with('E') || addr.starts_with('k'))
            && addr.len() == 47
        {
            "Did you miss the last symbol of the address (expected length is 48 but address length is 47)? "
        } else {
            ""
        };

        format!(
            "Address {} is not a valid address. {hint}Enter valid address in user-friendly {} or raw format {}",
            addr.yellow(),
            "EQ...".green(),
            "0:abcd...".green()
        )
    }

    #[must_use]
    pub fn port_bind_failure(server: &str, address: &str, flag: &str) -> String {
        format!(
            "Failed to start {server} on {address}\nChoose another port with {flag}\nOr stop the process currently listening on that port"
        )
    }

    #[must_use]
    pub fn script_not_found(config: &ActonConfig, name: &str) -> String {
        let Some(available) = available_scripts(config) else {
            return format!(
                "Script {} not found in Acton.toml. No scripts defined yet.

To define a new script add the following to Acton.toml:

{}

See https://ton-blockchain.github.io/acton/docs/commands/run for more information",
                name.yellow(),
                "[scripts]
script-name = \"command invocation\""
                    .green()
            );
        };

        format!(
            "Script {} not found in Acton.toml\nAvailable scripts:\n{}",
            name.yellow(),
            available
        )
    }

    #[must_use]
    pub fn available_scripts(config: &ActonConfig) -> Option<String> {
        let Some(scripts) = &config.scripts else {
            return None;
        };

        if scripts.is_empty() {
            return None;
        }

        Some(
            scripts
                .keys()
                .map(|s| format!(" {}", s.yellow()))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    #[must_use]
    pub fn no_scripts_section() -> String {
        format!(
            "No {} section found in Acton.toml.\nTo add a script add the following section to Acton.toml:\n\n{}\n{}\n{}\n\nSee https://ton-blockchain.github.io/acton/docs/commands/run for more information",
            "[scripts]".yellow(),
            "[scripts]".green(),
            "deploy = \"acton script scripts/deploy.tolk --net testnet\"".green(),
            "test = \"acton test tests/unit\"".green()
        )
    }

    #[must_use]
    pub fn no_wallets_found() -> String {
        format!(
            "No wallets configured in {} or global.wallets.toml.\nTo add a wallet use {} or add the following to {} manually:\n\n{}\n{}\n{}\n{}\n\nSee https://ton-blockchain.github.io/acton/docs/tutorial/setup-wallets for more information",
            "wallets.toml".yellow(),
            "acton wallet new".yellow(),
            "wallets.toml".green(),
            "[wallets.deployer]".green(),
            "kind = \"v5r1\"".green(),
            "workchain = 0".green(),
            "keys = { mnemonic = \"...\" }".green()
        )
    }

    #[must_use]
    pub fn no_libraries_found() -> String {
        format!(
            "No libraries configured in {} or {}.\nTo add a library use {} or add a record to {} manually.",
            "libraries.toml".yellow(),
            "global.libraries.toml".yellow(),
            "acton library publish".yellow(),
            "libraries.toml".green()
        )
    }
}

pub fn select_contract(
    contract_id: Option<String>,
    config: &ActonConfig,
) -> anyhow::Result<String> {
    let contract_key = if let Some(id) = contract_id {
        id
    } else {
        let contracts = config.contracts().ok_or_else(|| {
            anyhow!("No contracts configured in Acton.toml. Please add a contract configuration.")
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
    };
    Ok(contract_key)
}

#[must_use]
pub const fn executor_verbosity_for_cli_level(level: u8) -> ExecutorVerbosity {
    match level {
        0 => ExecutorVerbosity::Off,
        _ => ExecutorVerbosity::Full,
    }
}

#[must_use]
pub const fn max_executor_verbosity(
    lhs: ExecutorVerbosity,
    rhs: ExecutorVerbosity,
) -> ExecutorVerbosity {
    if (lhs as i32) >= (rhs as i32) {
        lhs
    } else {
        rhs
    }
}

pub fn validate_cli_verbosity(level: u8) -> anyhow::Result<u8> {
    if level <= 1 {
        Ok(level)
    } else {
        anyhow::bail!(
            "Verbosity levels above 1 are not supported yet. Use --verbose at most once."
        );
    }
}

pub fn select_wallet(wallet_name: Option<String>, config: &ActonConfig) -> anyhow::Result<String> {
    let wallet_name = if let Some(name) = wallet_name {
        name
    } else {
        let wallets_config = config
            .wallets()
            .ok_or_else(|| anyhow!(error_fmt::no_wallets_found()))?;

        let wallet_names: Vec<&String> = wallets_config.keys().collect();
        match wallet_names.len() {
            0 => anyhow::bail!(error_fmt::no_wallets_found()),
            1 => wallet_names[0].clone(),
            _ if !stdin().is_terminal() || !stdout().is_terminal() => {
                let available_wallets = wallet_names
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!(
                    "Cannot prompt for wallet selection in a non-interactive environment.\n\nPass the wallet name explicitly. Available wallets: {}",
                    available_wallets.cyan()
                );
            }
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
        let symlink_path = configured_project_root().join("global.wallets.toml");
        if !symlink_path.exists() {
            create_symlink(&global_path, &symlink_path)?;
        }
    }
    Ok(())
}

pub fn symlink_global_libraries() -> anyhow::Result<()> {
    if let Some(global_path) = global_libraries_path()
        && global_path.exists()
    {
        let symlink_path = configured_project_root().join("global.libraries.toml");
        if !symlink_path.exists() {
            create_symlink(&global_path, &symlink_path)?;
        }
    }
    Ok(())
}
