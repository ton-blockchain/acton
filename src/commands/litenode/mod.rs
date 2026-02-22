use acton_config::color::OwoColorize;
use retrace::Network;
use std::str::FromStr;
use std::sync::Arc;
use ton_litenode::node::StateSource;
use ton_litenode::remote::RemoteProvider;
use ton_litenode::{LiteNode, ServerArgs, run_server};

pub async fn litenode_start_cmd(
    port: u16,
    db_path: Option<String>,
    fork_net: Option<String>,
    api_key: Option<String>,
) -> anyhow::Result<()> {
    let state_source = if let Some(network) = fork_net {
        let network = Network::from_str(&network)?;
        StateSource::Remote(RemoteProvider { network, api_key })
    } else {
        StateSource::Local
    };

    let node = Arc::new(LiteNode::new(state_source, db_path.clone()));
    run_server(node, ServerArgs { port, db_path }).await?;
    Ok(())
}

pub async fn litenode_airdrop_cmd(address: &str, amount_ton: f64, port: u16) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let amount_nanotons = (amount_ton * 1_000_000_000.0) as u128;

    let res = client
        .post(format!("http://localhost:{}/admin/faucet", port))
        .json(&serde_json::json!({
            "address": address,
            "amount": amount_nanotons
        }))
        .send()
        .await?;

    if res.status().is_success() {
        let json: serde_json::Value = res.json().await?;
        if json.get("ok").and_then(|v| v.as_bool()).unwrap_or(false)
            || json
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            println!(
                "{} airdrop {} TON to {} on localnet",
                "Successfully".green().bold(),
                amount_ton,
                address
            );
        } else {
            let error = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            anyhow::bail!("Airdrop failed: {}", error);
        }
    } else {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("Airdrop failed with status {}: {}", status, body);
    }

    Ok(())
}
