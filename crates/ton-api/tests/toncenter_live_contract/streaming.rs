use super::support::{Live, decode, fixture};
use anyhow::{Context, Result, bail};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use std::env;
use std::io::{BufRead, BufReader};
use ton_api::toncenter::streaming::v2::{
    self, EventType, Finality, Status, StatusResponse, Subscription, UnsubscribeRequest,
    WebSocketRequest,
};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, connect};

fn live() -> Result<Option<Live>> {
    Live::from_env()
}

fn subscription(address: String) -> Subscription {
    Subscription {
        types: vec![EventType::Transactions],
        addresses: vec![address],
        trace_external_hash_norms: Vec::new(),
        min_finality: Some(Finality::Finalized),
        action_types: Vec::new(),
        supported_action_types: Vec::new(),
        include_address_book: Some(true),
        include_metadata: Some(true),
    }
}

#[test]
#[ignore = "optional live TonCenter contract test; requires an API key"]
fn sse_subscription_request_and_status_response() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    if live.require_api_key().is_none() {
        return Ok(());
    }
    let address = fixture(&live)?.transaction.account.clone();
    let url = env::var("ACTON_TONCENTER_LIVE_SSE_URL").unwrap_or_else(|_| v2::SSE_URL.to_owned());
    let response = live.send_raw(
        live.post_request(url)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .json(&subscription(address)),
        "streaming SSE subscribe",
    )?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().context("failed to read SSE error body")?;
        bail!("streaming SSE subscribe returned HTTP {status}: {body}");
    }

    for line in BufReader::new(response).lines() {
        let line = line.context("failed to read SSE response line")?;
        let Some(payload) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if payload.is_empty() {
            continue;
        }
        let response: StatusResponse = decode(payload, "streaming SSE status", status)?;
        if response.status != Status::Subscribed {
            bail!("unexpected SSE subscription status: {:?}", response.status);
        }
        return Ok(());
    }
    bail!("SSE stream closed before the subscription status")
}

#[test]
#[ignore = "optional live TonCenter contract test; requires an API key"]
fn websocket_request_covers_ping_subscribe_and_unsubscribe() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let Some(api_key) = live.require_api_key() else {
        return Ok(());
    };
    let address = fixture(&live)?.transaction.account.clone();
    let base_url = env::var("ACTON_TONCENTER_LIVE_WEBSOCKET_URL")
        .unwrap_or_else(|_| v2::WEBSOCKET_URL.to_owned());
    let separator = if base_url.contains('?') { '&' } else { '?' };
    let url = format!(
        "{base_url}{separator}api_key={}",
        urlencoding::encode(api_key)
    );
    let request = url
        .into_client_request()
        .context("invalid live TonCenter WebSocket URL")?;
    live.wait_for_rate_limit()?;
    let (mut socket, _) = connect(request).context("live TonCenter WebSocket handshake failed")?;

    send_and_expect_status(
        &mut socket,
        &WebSocketRequest::Ping {
            id: Some("live-ping".to_owned()),
        },
        Status::Pong,
    )?;
    send_and_expect_status(
        &mut socket,
        &WebSocketRequest::Subscribe {
            id: Some("live-subscribe".to_owned()),
            subscription: subscription(address.clone()),
        },
        Status::Subscribed,
    )?;
    send_and_expect_status(
        &mut socket,
        &WebSocketRequest::Unsubscribe {
            id: Some("live-unsubscribe".to_owned()),
            request: UnsubscribeRequest {
                addresses: vec![address],
                trace_external_hash_norms: Vec::new(),
            },
        },
        Status::Unsubscribed,
    )?;
    socket.close(None).context("failed to close WebSocket")?;
    Ok(())
}

fn send_and_expect_status<S>(
    socket: &mut tungstenite::WebSocket<S>,
    request: &WebSocketRequest,
    expected: Status,
) -> Result<()>
where
    S: std::io::Read + std::io::Write,
{
    let json = serde_json::to_string(request).context("failed to serialize WebSocket request")?;
    socket
        .send(Message::Text(json.into()))
        .context("failed to send WebSocket request")?;
    let message = socket
        .read()
        .context("failed to read WebSocket status response")?;
    let text = message
        .into_text()
        .context("WebSocket status response is not text")?;
    let response: StatusResponse = serde_json::from_str(&text)
        .with_context(|| format!("WebSocket response does not match StatusResponse: {text}"))?;
    if response.status != expected {
        bail!(
            "unexpected WebSocket status: expected {expected:?}, got {:?}",
            response.status
        );
    }
    Ok(())
}
