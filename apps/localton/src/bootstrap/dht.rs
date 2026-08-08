//! Initialization and process command construction for the local DHT node.
//!
//! The first validator also owns the network's bootstrap DHT endpoint. This
//! module asks `dht-server` to create its database, publishes its UDP address,
//! builds the signed DHT node descriptors for `global.config.json`, and later
//! constructs the persistent DHT process command.

use std::{fs, time::Duration};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use tokio::process::Command;
use tracing::info;

use crate::{
    binaries::TonBinaries,
    runtime::run_checked,
    storage::NodeSettings,
    storage::{Layout, write_json_atomic},
};

use super::engine_config::patch_out_port;

/// Creates the persistent DHT database and returns signed node descriptors.
///
/// `dht-server` first generates a keyring and config for the configured UDP
/// endpoint. Each keyring entry is then combined with the advertised address by
/// `generate-random-id -m dht`, producing the JSON descriptor that peers read
/// from `global.config.json` to discover this local network.
pub(super) async fn initialize_dht(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    timeout: Duration,
) -> Result<Vec<Value>> {
    info!("initializing local DHT");
    let mut command = Command::new(binaries.command("dht-server"));
    command
        .args([
            "--verbosity",
            &node.verbosity.to_string(),
            "--threads",
            &node.threads.to_string(),
            "--global-config",
        ])
        .arg(&layout.global_config)
        .arg("--logname")
        .arg(layout.logs.join("dht-init"))
        .arg("--db")
        .arg(&layout.dht_db)
        .args(["-I", &format!("{}:{}", node.public_ip, node.dht_port)]);
    run_checked("dht-server initialization", command, timeout).await?;

    // The binary owns the config schema. Patch only the launcher-controlled
    // outbound port instead of replacing the generated document.
    let config_path = layout.dht_db.join("config.json");
    ensure!(
        config_path.is_file(),
        "DHT initialization did not create {}",
        config_path.display()
    );
    patch_out_port(&config_path, node.out_port)?;

    // The descriptor advertises the UDP address on which the local DHT accepts
    // ADNL datagrams. TON JSON represents IPv4 as one numeric 32-bit value.
    let address_list_path = layout.dht_db.join("adnl-address-list.json");
    write_json_atomic(
        &address_list_path,
        &json!({
            "@type": "adnl.addressList",
            "addrs": [{
                "@type": "adnl.address.udp",
                "ip": u32::from(node.public_ip) as i64,
                "port": node.dht_port,
            }],
            "version": 0,
            "reinit_date": 0,
            "priority": 0,
            "expire_at": 0,
        }),
    )?;

    // Publish every canonical 256-bit key created by dht-server instead of
    // relying on platform-specific directory order.
    let keyring = layout.dht_db.join("keyring");
    let mut nodes = Vec::new();
    for entry in fs::read_dir(&keyring)
        .with_context(|| format!("failed to read DHT keyring {}", keyring.display()))?
    {
        let path = entry?.path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().len() == 64)
        {
            let mut command = Command::new(binaries.command("generate-random-id"));
            command
                .args(["-m", "dht", "-k"])
                .arg(&path)
                .arg("-f")
                .arg(&address_list_path);
            let output = run_checked("DHT node descriptor generation", command, timeout).await?;
            nodes.push(parse_json_output(&output.stdout)?);
        }
    }
    ensure!(
        !nodes.is_empty(),
        "DHT initialization created no keyring keys"
    );
    Ok(nodes)
}

/// Builds the long-running DHT command for an initialized database.
///
/// Unlike [`initialize_dht`], this reopens the persistent keyring and config; it
/// must not create a new DHT identity on every launcher run.
pub(super) fn command(layout: &Layout, binaries: &TonBinaries, node: &NodeSettings) -> Command {
    let mut command = Command::new(binaries.command("dht-server"));
    command
        .args([
            "-v",
            &node.verbosity.to_string(),
            "-t",
            &node.threads.to_string(),
            "-C",
        ])
        .arg(&layout.global_config)
        .arg("-l")
        .arg(layout.logs.join("dht-engine"))
        .arg("-D")
        .arg(&layout.dht_db)
        .args(["-I", &format!("{}:{}", node.public_ip, node.dht_port)]);
    command
}

/// Extracts a descriptor even when a TON binary surrounds JSON with log lines.
fn parse_json_output(output: &str) -> Result<Value> {
    if let Ok(value) = serde_json::from_str(output.trim()) {
        return Ok(value);
    }
    let start = output
        .find('{')
        .context("command output contains no JSON object")?;
    let end = output
        .rfind('}')
        .context("command output contains no complete JSON object")?;
    serde_json::from_str(&output[start..=end]).context("invalid JSON in command output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_noisy_output() {
        let value = parse_json_output("log line\n{\"@type\":\"dht.node\"}\n").unwrap();
        assert_eq!(value["@type"], "dht.node");
    }
}
