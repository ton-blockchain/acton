//! Network selection happens before connecting to any service or changing Docker.

use acton_localnet::catalog::{self, NetworkDirectory};
use inquire::{Select, error::InquireError};
use std::{
    io::{IsTerminal, stdin, stdout},
    path::Path,
};

pub(super) fn interactive(json: bool) -> bool {
    !json && stdin().is_terminal() && stdout().is_terminal()
}

/// Names and IDs select the same persisted deployment. A missing name is safe
/// to infer only for a single network; scripts must disambiguate explicitly.
pub(super) async fn resolve(
    root: &Path,
    name: Option<&str>,
    json: bool,
) -> anyhow::Result<Option<NetworkDirectory>> {
    choose(catalog::list(root).await?, name, json)
}

pub(super) fn choose(
    mut networks: Vec<NetworkDirectory>,
    name: Option<&str>,
    json: bool,
) -> anyhow::Result<Option<NetworkDirectory>> {
    if let Some(name) = name {
        return networks
            .into_iter()
            .find(|n| n.network.name == name || n.network.id == name)
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!("Network {name} was not found"));
    }
    anyhow::ensure!(
        !networks.is_empty(),
        "No localnet networks found; run `acton localnet start <name>`"
    );
    if networks.len() == 1 {
        return Ok(networks.pop());
    }
    anyhow::ensure!(
        interactive(json),
        "Multiple localnet networks exist; pass a network name or ID in non-interactive mode"
    );

    let names = networks.iter().map(|n| n.network.name.clone()).collect();
    let Some(selected) = prompt(Select::new("Select a network", names).prompt())? else {
        return Ok(None);
    };
    Ok(networks.into_iter().find(|n| n.network.name == selected))
}

// Escape and Ctrl-C cancel before any operation has been submitted.
pub(super) fn prompt<T>(result: Result<T, InquireError>) -> anyhow::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            println!("Cancelled");
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}
