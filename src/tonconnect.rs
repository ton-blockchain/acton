use anyhow::{Context as AnyhowContext, anyhow};
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::oneshot;
use ton_api::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, ExactSize};
use tycho_types::models::{
    Base64StdAddrFlags, DisplayBase64StdAddr, IntAddr, OwnedRelaxedMessage, RelaxedMsgInfo,
    StdAddr, StdAddrFormat,
};

const TONCONNECT_MAINNET_CHAIN: &str = "-239";
const TONCONNECT_TESTNET_CHAIN: &str = "-3";

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Acton TON Connect</title>
  <script src="https://unpkg.com/@tonconnect/ui@latest/dist/tonconnect-ui.min.js"></script>
  <style>
    :root {
      color-scheme: light dark;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: #101418;
      color: #f5f7fa;
    }
    body {
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
    }
    main {
      width: min(520px, calc(100vw - 32px));
      padding: 28px;
      border: 1px solid #2b333d;
      border-radius: 8px;
      background: #151b22;
    }
    h1 {
      margin: 0 0 12px;
      font-size: 24px;
      line-height: 1.2;
      letter-spacing: 0;
    }
    p {
      margin: 0 0 20px;
      color: #b7c0cb;
      line-height: 1.5;
    }
    #status {
      min-height: 24px;
      margin-top: 18px;
      color: #d7dde5;
      font-size: 14px;
    }
  </style>
</head>
<body>
  <main>
    <h1>Acton TON Connect</h1>
    <p>Connect a wallet and approve transactions requested by the running Acton script.</p>
    <div id="ton-connect-button"></div>
    <div id="status">Waiting for wallet connection...</div>
  </main>
  <script>
    const statusEl = document.getElementById('status');
    const tonConnectUI = new TON_CONNECT_UI.TonConnectUI({
      manifestUrl: 'https://ton-connect.github.io/demo-dapp-with-react-ui/tonconnect-manifest.json',
      buttonRootId: 'ton-connect-button',
    });

    const setStatus = (text) => {
      statusEl.textContent = text;
    };

    const postJson = async (url, body) => {
      const response = await fetch(url, {
        method: 'POST',
        headers: {'content-type': 'application/json'},
        body: JSON.stringify(body),
      });
      if (!response.ok) {
        throw new Error(await response.text());
      }
    };

    const publishWallet = async (wallet) => {
      if (!wallet || !wallet.account || !wallet.account.address) {
        setStatus('Waiting for wallet connection...');
        return;
      }
      await postJson('/api/connect', {
        address: wallet.account.address,
        chain: wallet.account.chain,
      });
      setStatus(`Connected: ${wallet.account.address}`);
    };

    tonConnectUI.onStatusChange((wallet) => {
      publishWallet(wallet).catch((error) => setStatus(error.message));
    });

    tonConnectUI.connectionRestored.then(() => {
      if (tonConnectUI.wallet) {
        publishWallet(tonConnectUI.wallet).catch((error) => setStatus(error.message));
      }
    });

    const poll = async () => {
      try {
        const response = await fetch('/api/request');
        const request = await response.json();
        if (!request.pending) {
          setTimeout(poll, 500);
          return;
        }

        setStatus('Approve the transaction in your wallet...');
        try {
          const result = await tonConnectUI.sendTransaction(request.transaction);
          await postJson('/api/response', {id: request.id, ok: true, boc: result.boc});
          setStatus('Transaction approved. Waiting for the next request...');
        } catch (error) {
          await postJson('/api/response', {
            id: request.id,
            ok: false,
            error: error && error.message ? error.message : String(error),
          });
          setStatus('Transaction was rejected or failed.');
        }
      } catch (error) {
        setStatus(error && error.message ? error.message : String(error));
      }

      setTimeout(poll, 500);
    };

    poll();
  </script>
</body>
</html>
"#;

const ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
<rect width="64" height="64" rx="12" fill="#0f1f2e"/>
<path d="M14 16h36L32 50 14 16z" fill="#35a8ff"/>
<path d="M21 21h22L32 42 21 21z" fill="#fff" opacity=".86"/>
</svg>
"##;

#[derive(Clone)]
pub struct TonConnectContext {
    pub session: Arc<TonConnectSession>,
    pub wallet: TonConnectWallet,
}

#[derive(Clone, Debug)]
pub struct TonConnectWallet {
    pub address: StdAddr,
    pub chain: Option<String>,
}

pub struct TonConnectSession {
    state: Arc<TonConnectState>,
    url: String,
    shutdown: Option<oneshot::Sender<()>>,
    server_thread: Option<thread::JoinHandle<()>>,
}

struct TonConnectState {
    connected: Mutex<Option<TonConnectWallet>>,
    connected_cv: Condvar,
    pending: Mutex<Option<PendingTonConnectRequest>>,
    pending_cv: Condvar,
    next_request_id: AtomicU64,
}

struct PendingTonConnectRequest {
    id: u64,
    transaction: TonConnectTransaction,
    response: Option<Result<String, String>>,
}

#[derive(Clone)]
struct TonConnectWebState {
    inner: Arc<TonConnectState>,
    base_url: Arc<str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TonConnectTransaction {
    pub valid_until: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    pub messages: Vec<TonConnectMessage>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TonConnectMessage {
    pub address: String,
    pub amount: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_init: Option<String>,
}

#[derive(Deserialize)]
struct ConnectPayload {
    address: String,
    chain: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TonConnectManifest {
    url: String,
    name: &'static str,
    icon_url: String,
}

#[derive(Serialize)]
struct RequestPollResponse {
    pending: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transaction: Option<TonConnectTransaction>,
}

#[derive(Deserialize)]
struct ResponsePayload {
    id: u64,
    ok: bool,
    boc: Option<String>,
    error: Option<String>,
}

impl TonConnectSession {
    pub fn start() -> anyhow::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .context("Failed to bind local TON Connect server")?;
        listener
            .set_nonblocking(true)
            .context("Failed to configure local TON Connect server socket")?;
        let addr = listener
            .local_addr()
            .context("Failed to read local TON Connect server address")?;
        let url = format!("http://{addr}");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let state = Arc::new(TonConnectState {
            connected: Mutex::new(None),
            connected_cv: Condvar::new(),
            pending: Mutex::new(None),
            pending_cv: Condvar::new(),
            next_request_id: AtomicU64::new(1),
        });
        let web_state = TonConnectWebState {
            inner: Arc::clone(&state),
            base_url: Arc::<str>::from(url.as_str()),
        };

        let app = Router::new()
            .route("/", get(index))
            .route("/icon.svg", get(icon))
            .route("/tonconnect-manifest.json", get(manifest))
            .route("/api/connect", post(connect))
            .route("/api/request", get(request))
            .route("/api/response", post(response))
            .with_state(web_state);

        let server_thread = thread::Builder::new()
            .name("acton-tonconnect".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("acton-tonconnect-runtime")
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        eprintln!("Failed to start TON Connect runtime: {error}");
                        return;
                    }
                };

                runtime.block_on(async move {
                    let listener = match tokio::net::TcpListener::from_std(listener) {
                        Ok(listener) => listener,
                        Err(error) => {
                            eprintln!("Failed to start TON Connect listener: {error}");
                            return;
                        }
                    };

                    if let Err(error) = axum::serve(listener, app)
                        .with_graceful_shutdown(async {
                            let _ = shutdown_rx.await;
                        })
                        .await
                    {
                        eprintln!("TON Connect server stopped with an error: {error}");
                    }
                });
            })
            .context("Failed to spawn local TON Connect server")?;

        Ok(Self {
            state,
            url,
            shutdown: Some(shutdown_tx),
            server_thread: Some(server_thread),
        })
    }

    pub fn connect(&self, network: &Network) -> anyhow::Result<TonConnectWallet> {
        println!("TON Connect page: {}", self.url);
        if let Err(error) = opener::open(&self.url) {
            eprintln!("Failed to open browser automatically: {error}");
            eprintln!("Open the TON Connect page manually: {}", self.url);
        }
        println!("Waiting for TON Connect wallet...");

        let mut connected = self
            .state
            .connected
            .lock()
            .expect("TON Connect connected wallet mutex poisoned");
        while connected.is_none() {
            connected = self
                .state
                .connected_cv
                .wait(connected)
                .expect("TON Connect connected wallet mutex poisoned");
        }

        let wallet = connected
            .clone()
            .expect("TON Connect wallet must be available after wait");
        drop(connected);
        validate_wallet_network(&wallet, network)?;
        Ok(wallet)
    }

    pub fn send_transaction(&self, transaction: TonConnectTransaction) -> anyhow::Result<String> {
        let id = self.state.next_request_id.fetch_add(1, Ordering::Relaxed);

        let mut pending = self
            .state
            .pending
            .lock()
            .expect("TON Connect pending request mutex poisoned");
        while pending.is_some() {
            pending = self
                .state
                .pending_cv
                .wait(pending)
                .expect("TON Connect pending request mutex poisoned");
        }

        *pending = Some(PendingTonConnectRequest {
            id,
            transaction,
            response: None,
        });
        self.state.pending_cv.notify_all();
        println!("Approve TON Connect transaction #{id} in your wallet...");

        loop {
            if let Some(request) = pending.as_mut()
                && request.id == id
                && let Some(response) = request.response.take()
            {
                *pending = None;
                self.state.pending_cv.notify_all();
                return response
                    .map_err(|error| anyhow!("TON Connect transaction failed: {error}"));
            }

            pending = self
                .state
                .pending_cv
                .wait(pending)
                .expect("TON Connect pending request mutex poisoned");
        }
    }
}

impl Drop for TonConnectSession {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.server_thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn ensure_supported_network(network: &Network) -> anyhow::Result<()> {
    tonconnect_chain(network).map(|_| ())
}

pub fn transaction_from_message(
    message: &Cell,
    network: &Network,
) -> anyhow::Result<TonConnectTransaction> {
    let chain = tonconnect_chain(network)?;
    let expired_at_time = std::time::SystemTime::now() + Duration::from_secs(600);
    let valid_until = expired_at_time.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(TonConnectTransaction {
        valid_until,
        network: Some(chain.to_string()),
        messages: vec![message_from_cell(message, network)?],
    })
}

fn message_from_cell(message: &Cell, network: &Network) -> anyhow::Result<TonConnectMessage> {
    let parsed = message
        .parse::<OwnedRelaxedMessage>()
        .context("Failed to parse internal message for TON Connect")?;
    let RelaxedMsgInfo::Int(info) = parsed.info else {
        anyhow::bail!("TON Connect can broadcast only internal wallet messages");
    };
    if !info.value.other.is_empty() {
        anyhow::bail!("TON Connect does not support extra currencies in wallet messages");
    }
    let IntAddr::Std(dest) = info.dst else {
        anyhow::bail!("TON Connect does not support variable destination addresses");
    };

    let payload = body_to_cell(parsed.body)?
        .filter(|cell| !is_empty_cell(cell))
        .map(|cell| Boc::encode_base64(&cell));
    let state_init = parsed
        .init
        .map(|state_init| CellBuilder::build_from(state_init).map(|cell| Boc::encode_base64(&cell)))
        .transpose()
        .context("Failed to serialize state init for TON Connect")?;

    Ok(TonConnectMessage {
        address: format_address(&dest, network, info.bounce),
        amount: info.value.tokens.to_string(),
        payload,
        state_init,
    })
}

fn body_to_cell(body: tycho_types::cell::CellSliceParts) -> anyhow::Result<Option<Cell>> {
    if body.exact_size().bits == 0 && body.exact_size().refs == 0 {
        return Ok(None);
    }

    let (range, cell) = body;
    let slice = range
        .apply(&cell)
        .context("Failed to extract message body for TON Connect")?;
    let mut builder = CellBuilder::new();
    builder
        .store_slice(slice)
        .context("Failed to serialize message body for TON Connect")?;
    Ok(Some(
        builder
            .build()
            .context("Failed to build message body for TON Connect")?,
    ))
}

fn is_empty_cell(cell: &Cell) -> bool {
    cell.as_ref().bit_len() == 0 && cell.as_ref().reference_count() == 0
}

fn format_address(address: &StdAddr, network: &Network, bounceable: bool) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: network.uses_testnet_address_format(),
            base64_url: true,
            bounceable,
        },
    }
    .to_string()
}

fn validate_wallet_network(wallet: &TonConnectWallet, network: &Network) -> anyhow::Result<()> {
    let expected = tonconnect_chain(network)?;
    if wallet
        .chain
        .as_deref()
        .is_some_and(|chain| chain != expected)
    {
        let actual = wallet
            .chain
            .as_deref()
            .and_then(chain_name)
            .unwrap_or("unknown");
        let expected_name = chain_name(expected).unwrap_or("unknown");
        anyhow::bail!(
            "Connected TON Connect wallet is on {actual}, but --net {expected_name} was requested. Switch the wallet network and run the script again."
        );
    }

    Ok(())
}

fn tonconnect_chain(network: &Network) -> anyhow::Result<&'static str> {
    match network {
        Network::Mainnet => Ok(TONCONNECT_MAINNET_CHAIN),
        Network::Testnet => Ok(TONCONNECT_TESTNET_CHAIN),
        Network::Localnet | Network::Custom(_) => anyhow::bail!(
            "`--tonconnect` supports only `--net mainnet` and `--net testnet`; use configured local wallets for {network}"
        ),
    }
}

fn chain_name(chain: &str) -> Option<&'static str> {
    match chain {
        TONCONNECT_MAINNET_CHAIN => Some("mainnet"),
        TONCONNECT_TESTNET_CHAIN => Some("testnet"),
        _ => None,
    }
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn icon() -> Response {
    let mut response = ICON_SVG.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("image/svg+xml; charset=utf-8"),
    );
    response
}

async fn manifest(State(state): State<TonConnectWebState>) -> Json<TonConnectManifest> {
    Json(TonConnectManifest {
        url: state.base_url.to_string(),
        name: "Acton",
        icon_url: format!("{}/icon.svg", state.base_url),
    })
}

async fn connect(
    State(state): State<TonConnectWebState>,
    Json(payload): Json<ConnectPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let (address, _) =
        StdAddr::from_str_ext(&payload.address, StdAddrFormat::any()).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid TON Connect wallet address: {error}"),
            )
        })?;
    let chain = payload.chain.as_ref().and_then(chain_value_to_string);

    {
        let mut connected = state
            .inner
            .connected
            .lock()
            .expect("TON Connect connected wallet mutex poisoned");
        *connected = Some(TonConnectWallet { address, chain });
    }
    state.inner.connected_cv.notify_all();
    Ok(StatusCode::NO_CONTENT)
}

async fn request(State(state): State<TonConnectWebState>) -> Json<RequestPollResponse> {
    let response = {
        let pending = state
            .inner
            .pending
            .lock()
            .expect("TON Connect pending request mutex poisoned");
        if let Some(request) = pending.as_ref()
            && request.response.is_none()
        {
            RequestPollResponse {
                pending: true,
                id: Some(request.id),
                transaction: Some(request.transaction.clone()),
            }
        } else {
            RequestPollResponse {
                pending: false,
                id: None,
                transaction: None,
            }
        }
    };

    Json(response)
}

async fn response(
    State(state): State<TonConnectWebState>,
    Json(payload): Json<ResponsePayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let mut pending = state
            .inner
            .pending
            .lock()
            .expect("TON Connect pending request mutex poisoned");
        let Some(request) = pending.as_mut().filter(|request| request.id == payload.id) else {
            return Err((
                StatusCode::NOT_FOUND,
                format!("Unknown TON Connect request {}", payload.id),
            ));
        };

        request.response = Some(if payload.ok {
            payload
                .boc
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        "TON Connect response is missing boc".to_string(),
                    )
                })
                .map(Ok)?
        } else {
            Err(payload
                .error
                .unwrap_or_else(|| "wallet rejected transaction".to_string()))
        });
        drop(pending);
    }
    state.inner.pending_cv.notify_all();
    Ok(StatusCode::NO_CONTENT)
}

fn chain_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tycho_types::cell::HashBytes;
    use tycho_types::models::{CurrencyCollection, RelaxedIntMsgInfo};

    #[test]
    fn tonconnect_rejects_localnet_and_custom_networks() {
        assert!(ensure_supported_network(&Network::Localnet).is_err());
        assert!(ensure_supported_network(&Network::Custom("sandbox".into())).is_err());
    }

    #[test]
    fn tonconnect_message_uses_bounceable_destination_and_payload() {
        let src = StdAddr::new(0, HashBytes([1; 32]));
        let dest = StdAddr::new(0, HashBytes([2; 32]));
        let body = CellBuilder::build_from(0x1234_u16).unwrap();
        let message = CellBuilder::build_from(OwnedRelaxedMessage {
            info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
                bounce: true,
                src: Some(IntAddr::Std(src)),
                dst: IntAddr::Std(dest.clone()),
                value: CurrencyCollection::new(123),
                ..Default::default()
            }),
            init: None,
            body: (tycho_types::cell::CellSliceRange::full(&body), body),
            layout: None,
        })
        .unwrap();

        let transaction = transaction_from_message(&message, &Network::Testnet).unwrap();
        let message = &transaction.messages[0];

        assert_eq!(
            transaction.network.as_deref(),
            Some(TONCONNECT_TESTNET_CHAIN)
        );
        assert_eq!(message.amount, "123");
        assert_eq!(
            message.address,
            format_address(&dest, &Network::Testnet, true)
        );
        assert!(message.payload.is_some());
    }
}
