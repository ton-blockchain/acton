use owo_colors::OwoColorize;
use std::sync::Arc;
use ton_litenode::{LiteNode, run_server};

pub async fn litenode_start_cmd(port: u16, ui: bool, ui_port: u16) -> anyhow::Result<()> {
    let node = Arc::new(LiteNode::new());
    run_server(node, port, ui, ui_port).await?;
    Ok(())
}

pub async fn litenode_airdrop_cmd(address: &str, amount_ton: f64, port: u16) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let amount_nanotons = (amount_ton * 1_000_000_000.0) as u128;

    let res = client
        .post(format!("http://localhost:{}/faucet", port))
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
