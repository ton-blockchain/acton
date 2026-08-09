use acton_config::color::OwoColorize;
use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct LocalnetStatusOutput {
    pub running: bool,
    pub port: u16,
    pub uptime_seconds: Option<u64>,
    pub last_block_seqno: Option<u64>,
    pub current_unix_time: Option<u32>,
    pub time_offset_seconds: Option<i64>,
    pub next_block_timestamp: Option<u32>,
    pub state_source: Option<String>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct LocalnetStatusEnvelope {
    ok: bool,
    result: Option<LocalnetStatusResult>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocalnetStatusResult {
    uptime_seconds: u64,
    last_block_seqno: u64,
    current_unix_time: u32,
    time_offset_seconds: i64,
    next_block_timestamp: Option<u32>,
    state_source: String,
    fork_network: Option<String>,
    fork_block_number: Option<u64>,
}

pub async fn localnet_status_cmd(
    port: u16,
    json: bool,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let client = crate::http::client_builder()
        .user_agent(crate::build_info::user_agent())
        .build()?;
    let stopped = LocalnetStatusOutput {
        running: false,
        port,
        uptime_seconds: None,
        last_block_seqno: None,
        current_unix_time: None,
        time_offset_seconds: None,
        next_block_timestamp: None,
        state_source: None,
        fork_network: None,
        fork_block_number: None,
    };
    let auth_token = super::resolve_localnet_auth_token(auth_token);
    let request = client.get(format!("http://127.0.0.1:{port}/acton_nodeInfo"));
    let output = match super::with_localnet_auth(request, auth_token.as_deref())
        .send()
        .await
    {
        Ok(response) => parse_status_response(response, port)
            .await?
            .unwrap_or(stopped),
        Err(err) if err.is_connect() => stopped,
        Err(err) => return Err(err).context("Failed to query localnet status"),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        print_localnet_status(&output);
    }
    Ok(())
}

async fn parse_status_response(
    response: reqwest::Response,
    port: u16,
) -> anyhow::Result<Option<LocalnetStatusOutput>> {
    let status = response.status();
    if !status.is_success() {
        return Ok(None);
    }

    let body = response
        .text()
        .await
        .context("Failed to read localnet status response")?;
    let Ok(payload) = serde_json::from_str::<LocalnetStatusEnvelope>(&body) else {
        return Ok(None);
    };
    if !payload.ok {
        let message = payload
            .error
            .unwrap_or_else(|| "Unknown localnet status error".to_owned());
        anyhow::bail!("Failed to query localnet status: {message}");
    }

    let result = payload
        .result
        .context("Localnet status response did not include result payload")?;

    Ok(Some(LocalnetStatusOutput {
        running: true,
        port,
        uptime_seconds: Some(result.uptime_seconds),
        last_block_seqno: Some(result.last_block_seqno),
        current_unix_time: Some(result.current_unix_time),
        time_offset_seconds: Some(result.time_offset_seconds),
        next_block_timestamp: result.next_block_timestamp,
        state_source: Some(result.state_source),
        fork_network: result.fork_network,
        fork_block_number: result.fork_block_number,
    }))
}

fn print_localnet_status(status: &LocalnetStatusOutput) {
    if !status.running {
        println!(
            "{} http://127.0.0.1:{}",
            "Stopped:".white().bold(),
            status.port,
        );
        return;
    }

    println!(
        "{} http://127.0.0.1:{}",
        "Running:".white().bold(),
        status.port,
    );
    if let Some(last_block_seqno) = status.last_block_seqno {
        println!(
            "{} {}",
            "Last block seqno:".white().bold(),
            last_block_seqno,
        );
    }
    if let Some(uptime_seconds) = status.uptime_seconds {
        println!("{} {}s", "Uptime:".white().bold(), uptime_seconds);
    }
    if let Some(current_unix_time) = status.current_unix_time {
        println!("{} {}", "Virtual time:".white().bold(), current_unix_time);
    }
    if let Some(time_offset_seconds) = status.time_offset_seconds {
        println!("{} {}s", "Time offset:".white().bold(), time_offset_seconds);
    }
    if let Some(next_block_timestamp) = status.next_block_timestamp {
        println!(
            "{} {}",
            "Next block timestamp:".white().bold(),
            next_block_timestamp
        );
    }

    let source = match (
        &status.state_source,
        &status.fork_network,
        status.fork_block_number,
    ) {
        (Some(state_source), Some(fork_network), Some(fork_block_number))
            if state_source == "remote" =>
        {
            format!("{fork_network} at seqno {fork_block_number}")
        }
        (Some(state_source), Some(fork_network), None) if state_source == "remote" => {
            fork_network.clone()
        }
        (Some(state_source), _, _) if state_source == "local" => "local genesis".to_owned(),
        (Some(state_source), _, _) => state_source.clone(),
        _ => "unknown".to_owned(),
    };
    println!("{} {}", "State source:".white().bold(), source);
}
