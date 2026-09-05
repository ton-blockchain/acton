//! Interactive deletion resolves a concrete deployment before confirming data loss.

use super::selection;
use acton_localnet::Network;
use inquire::Confirm;

/// Confirmation names the selected deployment before a service or Docker command
/// is started. Noninteractive callers must opt in with --yes.
pub(super) fn confirm(network: &Network, yes: bool, json: bool) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    anyhow::ensure!(
        selection::interactive(json),
        "Deletion requires confirmation; pass --yes to delete the network in non-interactive mode"
    );
    let question = format!(
        "Delete network {:?} and all its blockchain data and snapshots?",
        network.name
    );
    match selection::prompt(Confirm::new(&question).with_default(false).prompt())? {
        Some(true) => Ok(true),
        Some(false) => {
            println!("Network deletion cancelled");
            Ok(false)
        }
        None => Ok(false),
    }
}
