use acton_config::color::OwoColorize;
use anyhow::Context as AnyhowContext;
use inquire::Select;
use qrcode::{EcLevel, QrCode, render::unicode};
use reqwest::blocking::Client;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, ExactSize};
use rston::models::{
    Base64StdAddrFlags, DisplayBase64StdAddr, IntAddr, OwnedRelaxedMessage, RelaxedMsgInfo,
    StdAddr, StdAddrFormat,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::io::Read;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use ton_api::Network;
use ton_connect_core::{
    AccountAddress, BridgeSseDecoder, CellBoc, ClientId, ConnectEvent, ConnectItem,
    ConnectItemReply, ConnectLink, ConnectRequest, DecimalString, FriendlyAddress, HttpBridgeUrl,
    HttpsUrl, KnownAppRequest, NetworkId, NonEmptyVec, PersistedSessionKeyPair, RawMessage,
    RawTransactionPayload, ReturnStrategy, SendTransactionRequest, SessionCrypto, TonAddressItem,
    TransactionPayload, WalletMessage, WalletResponse, WalletResult,
};

const TONCONNECT_MAINNET_CHAIN: &str = "-239";
const TONCONNECT_TESTNET_CHAIN: &str = "-3";
const TONCONNECT_WALLETS_LIST_URL: &str = "https://config.ton.org/wallets-v2.json";
const TONCONNECT_DEFAULT_BRIDGE_URL: &str = "https://connect.ton.org/bridge";
const TONCONNECT_MANIFEST_URL: &str =
    "https://ton-blockchain.github.io/acton/tonconnect-manifest.json";
const TONCONNECT_LINK_BASE: &str = "tc://";
const TONCONNECT_MESSAGE_TTL_SECONDS: u32 = 300;
const MAX_SSE_EVENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_STORAGE_FILE_BYTES: u64 = 2 * 1024 * 1024;

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
    storage_path: PathBuf,
    http: Client,
    state: Mutex<TonConnectState>,
}

struct TonConnectState {
    crypto: SessionCrypto,
    bridge: Option<HttpBridgeUrl>,
    wallet: Option<TonConnectWallet>,
    peer_client_id: Option<ClientId>,
    last_event_id: Option<String>,
    next_request_id: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedTonConnectSession {
    key_pair: PersistedSessionKeyPair,
    bridge_url: String,
    peer_client_id: ClientId,
    wallet_address: String,
    wallet_chain: Option<String>,
    last_event_id: Option<String>,
    next_request_id: u64,
}

struct ReceivedWalletMessage {
    sender: ClientId,
    message: WalletMessage,
}

#[derive(Deserialize)]
struct WalletsListEntry {
    name: String,
    universal_url: Option<String>,
    bridge: Vec<WalletsListBridge>,
}

#[derive(Deserialize)]
struct WalletsListBridge {
    #[serde(rename = "type")]
    kind: String,
    url: Option<String>,
}

#[derive(Clone)]
struct TonConnectWalletOption {
    name: String,
    link_base: String,
    bridge: HttpBridgeUrl,
}

impl fmt::Display for TonConnectWalletOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)
    }
}

impl TonConnectSession {
    pub fn start(storage_path: PathBuf) -> anyhow::Result<Self> {
        let http = crate::http::blocking_client_builder()
            .connect_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("Failed to create TON Connect HTTP client")?;
        let state = load_session(&storage_path)?.map_or_else(
            || {
                Ok(TonConnectState {
                    crypto: SessionCrypto::generate()?,
                    bridge: None,
                    wallet: None,
                    peer_client_id: None,
                    last_event_id: None,
                    next_request_id: 1,
                })
            },
            restore_session,
        )?;

        Ok(Self {
            storage_path,
            http,
            state: Mutex::new(state),
        })
    }

    pub fn connect(&self, network: &Network) -> anyhow::Result<TonConnectWallet> {
        let mut state = self
            .state
            .lock()
            .expect("TON Connect session mutex poisoned");
        if let Some(wallet) = state.wallet.clone() {
            validate_wallet_network(&wallet, network)?;
            return Ok(wallet);
        }

        let request = connect_request(network)?;
        let selected_wallet = select_tonconnect_wallet(&self.http)?;
        state.bridge = Some(selected_wallet.bridge);
        let link = ConnectLink::connect(
            state.crypto.client_id(),
            request.clone(),
            ReturnStrategy::None,
            None,
            None,
        )
        .to_url(&selected_wallet.link_base)
        .context("Failed to create TON Connect link")?;
        print_connect_link(link.as_str())?;
        println!("Waiting for TON Connect wallet");

        loop {
            let received = self.read_wallet_message(&mut state, None)?;
            let WalletMessage::Event(event) = received.message else {
                continue;
            };
            event
                .validate_for_connect(&request, None)
                .context("Wallet returned an invalid TON Connect response")?;
            match event {
                ConnectEvent::Connect { payload, .. } => {
                    let account = payload
                        .items
                        .iter()
                        .find_map(|item| match item {
                            ConnectItemReply::TonAddress(account) => Some(account),
                            ConnectItemReply::TonProof(_) | ConnectItemReply::Error(_) => None,
                        })
                        .context("TON Connect response is missing wallet address")?;
                    let wallet = wallet_from_account(account)?;
                    validate_wallet_network(&wallet, network)?;
                    state.peer_client_id = Some(received.sender);
                    state.wallet = Some(wallet.clone());
                    self.persist_session(&state)?;
                    drop(state);
                    return Ok(wallet);
                }
                ConnectEvent::ConnectError { payload, .. } => {
                    let message = payload.message;
                    drop(state);
                    anyhow::bail!("TON Connect connection failed: {message}");
                }
                ConnectEvent::Disconnect { .. } => {
                    drop(state);
                    anyhow::bail!("TON Connect wallet disconnected before connection completed");
                }
            }
        }
    }

    pub fn send_transaction(
        &self,
        mut transaction: RawTransactionPayload,
    ) -> anyhow::Result<String> {
        let mut state = self
            .state
            .lock()
            .expect("TON Connect session mutex poisoned");
        let peer_client_id = state
            .peer_client_id
            .context("TON Connect wallet is not connected")?;
        let bridge = state
            .bridge
            .clone()
            .context("TON Connect wallet bridge is not selected")?;
        if transaction.from.is_none() {
            let wallet = state
                .wallet
                .as_ref()
                .context("TON Connect wallet is not connected")?;
            transaction.from = Some(
                AccountAddress::try_from(wallet.address.to_string())
                    .context("Failed to encode TON Connect sender address")?,
            );
        }
        let id = state.next_request_id.to_string();
        state.next_request_id = state.next_request_id.saturating_add(1);

        let request = KnownAppRequest::SendTransaction(SendTransactionRequest {
            id: id.clone(),
            payload: TransactionPayload::Raw(transaction),
        });
        let post = bridge
            .prepare_app_request_post(&state.crypto, peer_client_id, message_ttl(), None, &request)
            .context("Failed to encrypt TON Connect transaction")?;
        self.persist_session(&state)?;
        self.http
            .post(post.url().clone())
            .header(reqwest::header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(post.body().as_str().to_owned())
            .send()
            .context("Failed to send TON Connect transaction to bridge")?
            .error_for_status()
            .context("TON Connect bridge rejected transaction request")?;
        println!("Approve TON Connect transaction #{id} in your wallet");

        loop {
            let received = self.read_wallet_message(&mut state, Some(peer_client_id))?;
            match received.message {
                WalletMessage::Response(WalletResponse::Success(response)) if response.id == id => {
                    self.persist_session(&state)?;
                    let result = response.result;
                    drop(state);
                    return match result {
                        WalletResult::String(boc) => Ok(boc),
                        WalletResult::Object(_) => anyhow::bail!(
                            "TON Connect wallet returned an invalid sendTransaction result"
                        ),
                    };
                }
                WalletMessage::Response(WalletResponse::Error {
                    error,
                    id: response_id,
                }) if response_id == id => {
                    self.persist_session(&state)?;
                    let message = error.message;
                    drop(state);
                    anyhow::bail!("TON Connect transaction failed: {message}");
                }
                WalletMessage::Event(ConnectEvent::Disconnect { .. }) => {
                    state.bridge = None;
                    state.wallet = None;
                    state.peer_client_id = None;
                    remove_session(&self.storage_path)?;
                    drop(state);
                    anyhow::bail!("TON Connect wallet disconnected");
                }
                WalletMessage::Event(ConnectEvent::ConnectError { payload, .. }) => {
                    let message = payload.message;
                    drop(state);
                    anyhow::bail!("TON Connect session failed: {message}");
                }
                WalletMessage::Event(ConnectEvent::Connect { .. }) | WalletMessage::Response(_) => {
                }
            }
        }
    }

    fn read_wallet_message(
        &self,
        state: &mut TonConnectState,
        expected_peer: Option<ClientId>,
    ) -> anyhow::Result<ReceivedWalletMessage> {
        let bridge = state
            .bridge
            .clone()
            .context("TON Connect wallet bridge is not selected")?;
        loop {
            let endpoint = bridge.events_endpoint(
                state.crypto.client_id(),
                state.last_event_id.as_deref(),
                None,
            );
            let mut response = self
                .http
                .get(endpoint)
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .send()
                .with_context(|| {
                    format!(
                        "Failed to connect to TON Connect bridge {}",
                        bridge.as_str()
                    )
                })?
                .error_for_status()
                .with_context(|| {
                    format!(
                        "TON Connect bridge {} rejected event subscription",
                        bridge.as_str()
                    )
                })?;
            let mut decoder = BridgeSseDecoder::new(max_sse_event_bytes());
            let mut chunk = [0_u8; 8192];

            loop {
                let read = response.read(&mut chunk).with_context(|| {
                    format!(
                        "Failed to read TON Connect bridge {} events",
                        bridge.as_str()
                    )
                })?;
                if read == 0 {
                    break;
                }
                for event in decoder
                    .push(&chunk[..read])
                    .context("Failed to decode TON Connect bridge event")?
                {
                    let sender = event.message().from();
                    if let Some(event_id) = event.event_id() {
                        state.last_event_id = Some(event_id.to_owned());
                    }
                    if expected_peer.is_some_and(|expected| sender != expected) {
                        continue;
                    }
                    let plaintext = event
                        .decrypt::<Value>(&state.crypto, sender)
                        .context("Failed to decrypt TON Connect wallet message")?;
                    let message = decode_wallet_message(plaintext)
                        .context("Failed to decode TON Connect wallet message")?;
                    return Ok(ReceivedWalletMessage { sender, message });
                }
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    }

    fn persist_session(&self, state: &TonConnectState) -> anyhow::Result<()> {
        let Some(wallet) = state.wallet.as_ref() else {
            return Ok(());
        };
        let peer_client_id = state
            .peer_client_id
            .context("TON Connect wallet peer is missing")?;
        let bridge = state
            .bridge
            .as_ref()
            .context("TON Connect wallet bridge is missing")?;
        let persisted = PersistedTonConnectSession {
            key_pair: state.crypto.persisted_keypair(),
            bridge_url: bridge.as_str().to_owned(),
            peer_client_id,
            wallet_address: wallet.address.to_string(),
            wallet_chain: wallet.chain.clone(),
            last_event_id: state.last_event_id.clone(),
            next_request_id: state.next_request_id,
        };
        write_session(&self.storage_path, &persisted)
    }
}

fn select_tonconnect_wallet(http: &Client) -> anyhow::Result<TonConnectWalletOption> {
    let entries = http
        .get(TONCONNECT_WALLETS_LIST_URL)
        .timeout(Duration::from_secs(15))
        .send()
        .context("Failed to load TON Connect wallets list")?
        .error_for_status()
        .context("TON Connect wallets list request failed")?
        .json::<Vec<WalletsListEntry>>()
        .context("Failed to parse TON Connect wallets list")?;
    let mut wallets = tonconnect_wallet_options(entries);
    wallets.push(TonConnectWalletOption {
        name: "Other wallet via connect.ton.org".to_owned(),
        link_base: TONCONNECT_LINK_BASE.to_owned(),
        bridge: HttpBridgeUrl::try_from(TONCONNECT_DEFAULT_BRIDGE_URL)
            .context("Failed to configure default TON Connect bridge")?,
    });

    Select::new("TON Connect wallet:", wallets)
        .prompt()
        .context("Failed to select TON Connect wallet")
}

fn tonconnect_wallet_options(entries: Vec<WalletsListEntry>) -> Vec<TonConnectWalletOption> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let link_base = entry.universal_url?;
            let bridge_url = entry
                .bridge
                .into_iter()
                .find(|bridge| bridge.kind == "sse")?
                .url?;
            let bridge = HttpBridgeUrl::try_from(bridge_url.as_str()).ok()?;
            Some(TonConnectWalletOption {
                name: entry.name,
                link_base,
                bridge,
            })
        })
        .collect()
}

fn decode_wallet_message(mut value: Value) -> anyhow::Result<WalletMessage> {
    normalize_wallet_message(&mut value);
    if value.get("event").is_some() {
        serde_json::from_value::<ConnectEvent>(value)
            .map(WalletMessage::Event)
            .context("invalid TON Connect wallet event")
    } else {
        serde_json::from_value::<WalletResponse>(value)
            .map(WalletMessage::Response)
            .context("invalid TON Connect wallet response")
    }
}

fn normalize_wallet_message(value: &mut Value) {
    if value.get("event").and_then(Value::as_str) != Some("connect") {
        return;
    }
    let Some(features) = value
        .pointer_mut("/payload/device/features")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let has_detailed_send_transaction = features
        .iter()
        .any(|feature| feature.get("name").and_then(Value::as_str) == Some("SendTransaction"));
    if has_detailed_send_transaction {
        // Reference wallets publish the legacy alias beside the detailed capability.
        features.retain(|feature| feature.as_str() != Some("SendTransaction"));
    }
}

pub fn ensure_supported_network(network: &Network) -> anyhow::Result<()> {
    tonconnect_chain(network).map(|_| ())
}

pub fn session_storage_path(project_root: &Path, network: &Network) -> anyhow::Result<PathBuf> {
    Ok(project_root
        .join("build")
        .join("sessions")
        .join("tonconnect")
        .join(format!("{}.json", tonconnect_network_name(network)?)))
}

pub fn transaction_from_message(
    message: &Cell,
    network: &Network,
) -> anyhow::Result<RawTransactionPayload> {
    let chain = tonconnect_chain(network)?;
    let expired_at_time = std::time::SystemTime::now() + Duration::from_secs(600);
    let valid_until = expired_at_time.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(RawTransactionPayload {
        valid_until: Some(valid_until),
        network: Some(NetworkId::try_from(chain)?),
        from: None,
        messages: NonEmptyVec::try_from(vec![message_from_cell(message, network)?])?,
    })
}

fn message_from_cell(message: &Cell, network: &Network) -> anyhow::Result<RawMessage> {
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
        .map(|cell| CellBoc::try_from(Boc::encode_base64(&cell)))
        .transpose()
        .context("Failed to validate message body for TON Connect")?;
    let state_init = parsed
        .init
        .map(|state_init| {
            CellBuilder::build_from(state_init)
                .map(|cell| Boc::encode_base64(&cell))
                .map_err(anyhow::Error::from)
                .and_then(|boc| CellBoc::try_from(boc).map_err(anyhow::Error::from))
        })
        .transpose()
        .context("Failed to serialize state init for TON Connect")?;

    Ok(RawMessage {
        address: FriendlyAddress::try_from(format_address(&dest, network, info.bounce))
            .context("Failed to encode TON Connect destination address")?,
        amount: DecimalString::try_from(info.value.tokens.to_string())
            .context("Failed to encode TON Connect amount")?,
        payload,
        state_init,
        extra_currency: None,
    })
}

fn body_to_cell(body: rston::cell::CellSliceParts) -> anyhow::Result<Option<Cell>> {
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
            "Connected TON Connect wallet is on {actual}, but {} was requested. Switch the wallet network and run the script again.",
            format!("--net {expected_name}").yellow()
        );
    }

    Ok(())
}

fn tonconnect_chain(network: &Network) -> anyhow::Result<&'static str> {
    match network {
        Network::Mainnet => Ok(TONCONNECT_MAINNET_CHAIN),
        Network::Testnet => Ok(TONCONNECT_TESTNET_CHAIN),
        Network::Localnet | Network::Custom(_) => anyhow::bail!(
            "{} supports only {} and {}; use configured local wallets for {network}",
            "--tonconnect".yellow(),
            "--net mainnet".yellow(),
            "--net testnet".yellow()
        ),
    }
}

fn tonconnect_network_name(network: &Network) -> anyhow::Result<&'static str> {
    let chain = tonconnect_chain(network)?;
    Ok(chain_name(chain).expect("supported TON Connect chain must have a network name"))
}

fn chain_name(chain: &str) -> Option<&'static str> {
    match chain {
        TONCONNECT_MAINNET_CHAIN => Some("mainnet"),
        TONCONNECT_TESTNET_CHAIN => Some("testnet"),
        _ => None,
    }
}

fn connect_request(network: &Network) -> anyhow::Result<ConnectRequest> {
    Ok(ConnectRequest {
        manifest_url: HttpsUrl::try_from(TONCONNECT_MANIFEST_URL)?,
        items: NonEmptyVec::try_from(vec![ConnectItem::from(TonAddressItem {
            network: Some(NetworkId::try_from(tonconnect_chain(network)?)?),
        })])?,
    })
}

fn wallet_from_account(
    account: &ton_connect_core::TonAddressItemReply,
) -> anyhow::Result<TonConnectWallet> {
    let (address, _) = StdAddr::from_str_ext(&account.address.to_string(), StdAddrFormat::any())
        .context("Wallet returned an invalid TON address")?;
    Ok(TonConnectWallet {
        address,
        chain: Some(account.network.as_str().to_owned()),
    })
}

fn print_connect_link(link: &str) -> anyhow::Result<()> {
    println!("Scan this QR code with a TON Connect wallet");
    let code = QrCode::with_error_correction_level(link.as_bytes(), EcLevel::L)
        .context("Failed to generate TON Connect QR code")?;
    println!(
        "{}",
        code.render::<unicode::Dense1x2>()
            .quiet_zone(true)
            .module_dimensions(1, 1)
            .build()
    );
    println!("TON Connect link: {link}");
    Ok(())
}

const fn max_sse_event_bytes() -> NonZeroUsize {
    NonZeroUsize::new(MAX_SSE_EVENT_BYTES).expect("TON Connect SSE limit must be non-zero")
}

const fn message_ttl() -> NonZeroU32 {
    NonZeroU32::new(TONCONNECT_MESSAGE_TTL_SECONDS)
        .expect("TON Connect message TTL must be non-zero")
}

fn load_session(path: &Path) -> anyhow::Result<Option<PersistedTonConnectSession>> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(path).with_context(|| {
        format!(
            "Failed to inspect TON Connect session storage {}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_STORAGE_FILE_BYTES {
        anyhow::bail!(
            "TON Connect session storage {} is too large",
            path.display()
        );
    }

    let content = fs::read_to_string(path).with_context(|| {
        format!(
            "Failed to read TON Connect session storage {}",
            path.display()
        )
    })?;
    if content.trim().is_empty() {
        return Ok(None);
    }

    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse TON Connect session {}", path.display()))
        .map(Some)
}

fn restore_session(persisted: PersistedTonConnectSession) -> anyhow::Result<TonConnectState> {
    let crypto = SessionCrypto::from_persisted(&persisted.key_pair)
        .context("Failed to restore TON Connect session keys")?;
    let bridge = HttpBridgeUrl::try_from(persisted.bridge_url.as_str())
        .context("Failed to restore TON Connect bridge")?;
    let (address, _) = StdAddr::from_str_ext(&persisted.wallet_address, StdAddrFormat::any())
        .context("Failed to restore TON Connect wallet address")?;
    Ok(TonConnectState {
        crypto,
        bridge: Some(bridge),
        wallet: Some(TonConnectWallet {
            address,
            chain: persisted.wallet_chain,
        }),
        peer_client_id: Some(persisted.peer_client_id),
        last_event_id: persisted.last_event_id,
        next_request_id: persisted.next_request_id.max(1),
    })
}

fn write_session(path: &Path, session: &PersistedTonConnectSession) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create TON Connect session storage directory {}",
                parent.display()
            )
        })?;
    }

    let content =
        serde_json::to_vec_pretty(session).context("Failed to serialize TON Connect session")?;
    if content.len() as u64 > MAX_STORAGE_FILE_BYTES {
        anyhow::bail!(
            "TON Connect session storage {} is too large",
            path.display()
        );
    }
    fs::write(path, content).with_context(|| {
        format!(
            "Failed to write TON Connect session storage {}",
            path.display()
        )
    })?;
    set_storage_permissions(path)?;
    Ok(())
}

fn remove_session(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to remove TON Connect session {}", path.display())),
    }
}

#[cfg(unix)]
fn set_storage_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).with_context(|| {
        format!(
            "Failed to restrict TON Connect session storage permissions {}",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn set_storage_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rston::cell::HashBytes;
    use rston::models::{CurrencyCollection, RelaxedIntMsgInfo};

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
            body: (rston::cell::CellSliceRange::full(&body), body),
            layout: None,
        })
        .unwrap();

        let transaction = transaction_from_message(&message, &Network::Testnet).unwrap();
        let message = &transaction.messages.as_slice()[0];

        assert_eq!(
            transaction.network.as_ref().map(NetworkId::as_str),
            Some(TONCONNECT_TESTNET_CHAIN)
        );
        assert_eq!(message.amount.as_str(), "123");
        assert_eq!(
            message.address.as_str(),
            format_address(&dest, &Network::Testnet, true)
        );
        assert!(message.payload.is_some());

        let encoded = ton_connect_core::AppRequest::encode(KnownAppRequest::SendTransaction(
            SendTransactionRequest {
                id: "1".to_owned(),
                payload: TransactionPayload::Raw(transaction),
            },
        ))
        .unwrap();
        assert!(matches!(
            encoded.decode().unwrap(),
            KnownAppRequest::SendTransaction(_)
        ));
    }

    #[test]
    fn tonconnect_link_contains_native_protocol_request() {
        let crypto = SessionCrypto::generate().unwrap();
        let request = connect_request(&Network::Testnet).unwrap();
        let link = ConnectLink::connect(
            crypto.client_id(),
            request,
            ReturnStrategy::None,
            None,
            None,
        )
        .to_url(TONCONNECT_LINK_BASE)
        .unwrap();

        assert_eq!(link.scheme(), "tc");
        assert_eq!(
            link.query_pairs().find(|(key, _)| key == "v").unwrap().1,
            "2"
        );
        assert!(link.query_pairs().any(|(key, value)| {
            key == "r"
                && value.contains(TONCONNECT_MANIFEST_URL)
                && value.contains(TONCONNECT_TESTNET_CHAIN)
        }));
        let compact = QrCode::with_error_correction_level(link.as_str(), EcLevel::L).unwrap();
        let default = QrCode::new(link.as_str()).unwrap();
        assert!(compact.width() < default.width());
    }

    #[test]
    fn tonconnect_session_storage_path_is_project_local_and_network_scoped() {
        let root = Path::new("/tmp/acton-project");

        assert_eq!(
            session_storage_path(root, &Network::Mainnet).unwrap(),
            root.join("build")
                .join("sessions")
                .join("tonconnect")
                .join("mainnet.json")
        );
        assert_eq!(
            session_storage_path(root, &Network::Testnet).unwrap(),
            root.join("build")
                .join("sessions")
                .join("tonconnect")
                .join("testnet.json")
        );
    }

    #[test]
    fn tonconnect_session_roundtrips_native_state() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("build/sessions/tonconnect/testnet.json");
        let wallet = StdAddr::new(0, HashBytes([3; 32]));
        let crypto = SessionCrypto::generate().unwrap();
        let client_id = crypto.client_id();
        let persisted = PersistedTonConnectSession {
            key_pair: crypto.persisted_keypair(),
            bridge_url: TONCONNECT_DEFAULT_BRIDGE_URL.to_owned(),
            peer_client_id: ClientId::from_bytes([4; 32]),
            wallet_address: wallet.to_string(),
            wallet_chain: Some(TONCONNECT_TESTNET_CHAIN.to_owned()),
            last_event_id: Some("42".to_owned()),
            next_request_id: 7,
        };

        write_session(&path, &persisted).unwrap();
        let restored = restore_session(load_session(&path).unwrap().unwrap()).unwrap();

        assert_eq!(restored.crypto.client_id(), client_id);
        assert_eq!(
            restored.bridge.as_ref().map(HttpBridgeUrl::as_str),
            Some(TONCONNECT_DEFAULT_BRIDGE_URL)
        );
        assert_eq!(restored.wallet.unwrap().address, wallet);
        assert_eq!(restored.last_event_id.as_deref(), Some("42"));
        assert_eq!(restored.next_request_id, 7);
    }

    #[test]
    fn tonconnect_storage_rejects_oversized_files() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("build/sessions/tonconnect/testnet.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, vec![b'x'; MAX_STORAGE_FILE_BYTES as usize + 1]).unwrap();

        let error = match load_session(&path) {
            Ok(_) => panic!("oversized session file was accepted"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("is too large"));
    }

    #[cfg(unix)]
    #[test]
    fn tonconnect_storage_file_is_private_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("build/sessions/tonconnect/testnet.json");

        let crypto = SessionCrypto::generate().unwrap();
        write_session(
            &path,
            &PersistedTonConnectSession {
                key_pair: crypto.persisted_keypair(),
                bridge_url: TONCONNECT_DEFAULT_BRIDGE_URL.to_owned(),
                peer_client_id: ClientId::from_bytes([4; 32]),
                wallet_address: StdAddr::new(0, HashBytes([3; 32])).to_string(),
                wallet_chain: Some(TONCONNECT_TESTNET_CHAIN.to_owned()),
                last_event_id: None,
                next_request_id: 1,
            },
        )
        .unwrap();

        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn tonconnect_wallets_list_selects_wallet_specific_link_and_bridge() {
        let entries = serde_json::from_value::<Vec<WalletsListEntry>>(serde_json::json!([{
            "name": "Tonkeeper",
            "universal_url": "https://app.tonkeeper.com/ton-connect",
            "bridge": [
                { "type": "sse", "url": "https://bridge.tonapi.io/bridge" },
                { "type": "js", "key": "tonkeeper" }
            ]
        }]))
        .unwrap();

        let wallets = tonconnect_wallet_options(entries);
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].name, "Tonkeeper");
        assert_eq!(
            wallets[0].link_base,
            "https://app.tonkeeper.com/ton-connect"
        );
        assert_eq!(
            wallets[0].bridge.as_str(),
            "https://bridge.tonapi.io/bridge"
        );
    }

    #[test]
    fn tonkeeper_legacy_and_detailed_send_transaction_features_are_accepted() {
        let message = serde_json::json!({
            "event": "connect",
            "id": 1,
            "payload": {
                "items": [],
                "device": {
                    "platform": "browser",
                    "appName": "tonkeeper",
                    "appVersion": "1",
                    "maxProtocolVersion": 2,
                    "features": [
                        "SendTransaction",
                        {
                            "name": "SendTransaction",
                            "maxMessages": 255,
                            "extraCurrencySupported": true
                        },
                        {
                            "name": "SignData",
                            "types": ["text", "binary", "cell"]
                        }
                    ]
                }
            }
        });

        assert!(serde_json::from_value::<WalletMessage>(message.clone()).is_err());
        assert!(matches!(
            decode_wallet_message(message),
            Ok(WalletMessage::Event(ConnectEvent::Connect { .. }))
        ));
    }
}
