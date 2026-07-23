use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use ton::{
    block_tlb::{CommonMsgInfoInt, Msg},
    ton_core::{
        cell::TonCell,
        traits::tlb::TLB,
        types::{TonAddress, tlb_core::TLBCoins},
    },
    ton_wallet::{Mnemonic, TonWallet, WALLET_ID_DEFAULT, WALLET_V5R1_ID_DEFAULT, WalletVersion},
};
use zeroize::Zeroizing;

const KEYRING_SERVICE: &str = "org.ton.acton.dev-wallet";
const TONCENTER_KEYRING_ACCOUNT: &str = "toncenter-api-key";
const TONCENTER_DOTENV_KEY: &str = "VITE_EXPLORER_TONCENTER_API_KEY";
const WALLETS_FILE: &str = "wallets.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletRecord {
    id: String,
    name: String,
    address: String,
    public_key: String,
    version: String,
    network: String,
    created_at: String,
}

#[derive(Default)]
struct WalletVault {
    operation_lock: Mutex<()>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GramTransferRequest {
    wallet_id: String,
    recipient: String,
    amount_nano: String,
    comment: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GramTransferResult {
    message_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletBalanceRequest {
    wallet_id: String,
}

#[derive(Debug, Serialize)]
struct WalletBalanceResult {
    balance: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletActivityRequest {
    wallet_id: String,
    limit: Option<u8>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WalletActivityItem {
    hash: String,
    timestamp: u64,
    direction: String,
    value_nano: String,
    fee_nano: String,
    counterparty: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToncenterEnvelope<T> {
    ok: bool,
    result: Option<T>,
    error: Option<String>,
}

#[tauri::command]
fn list_wallets(
    app: AppHandle,
    vault: State<'_, WalletVault>,
) -> Result<Vec<WalletRecord>, String> {
    let _guard = vault
        .operation_lock
        .lock()
        .map_err(|_| "Device vault lock is poisoned".to_owned())?;
    read_wallets(&wallets_path(&app)?)
}

#[tauri::command]
fn save_wallet(
    app: AppHandle,
    vault: State<'_, WalletVault>,
    record: WalletRecord,
    mnemonic: String,
) -> Result<(), String> {
    let _guard = vault
        .operation_lock
        .lock()
        .map_err(|_| "Device vault lock is poisoned".to_owned())?;

    if mnemonic.split_whitespace().count() != 24 {
        return Err("A TON mnemonic must contain exactly 24 words".to_owned());
    }

    let entry = keyring_entry(&record.id)?;
    entry
        .set_password(&mnemonic)
        .map_err(|error| format!("Failed to save mnemonic in the system keychain: {error}"))?;

    let path = wallets_path(&app)?;
    let mut wallets = read_wallets(&path)?;
    wallets.retain(|wallet| wallet.id != record.id);
    wallets.push(record.clone());

    if let Err(error) = write_wallets(&path, &wallets) {
        let _ = entry.delete_credential();
        return Err(error);
    }

    Ok(())
}

#[tauri::command]
fn remove_wallet(
    app: AppHandle,
    vault: State<'_, WalletVault>,
    wallet_id: String,
) -> Result<(), String> {
    let _guard = vault
        .operation_lock
        .lock()
        .map_err(|_| "Device vault lock is poisoned".to_owned())?;

    let path = wallets_path(&app)?;
    let mut wallets = read_wallets(&path)?;
    wallets.retain(|wallet| wallet.id != wallet_id);
    write_wallets(&path, &wallets)?;

    match keyring_entry(&wallet_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Wallet metadata was removed, but the keychain entry could not be deleted: {error}"
        )),
    }
}

#[tauri::command]
async fn get_wallet_balance(
    app: AppHandle,
    vault: State<'_, WalletVault>,
    request: WalletBalanceRequest,
) -> Result<WalletBalanceResult, String> {
    let record = {
        let _guard = vault
            .operation_lock
            .lock()
            .map_err(|_| "Device vault lock is poisoned".to_owned())?;
        read_wallets(&wallets_path(&app)?)?
            .into_iter()
            .find(|wallet| wallet.id == request.wallet_id)
            .ok_or_else(|| "This wallet is not available on this device".to_owned())?
    };
    let endpoint = toncenter_endpoint(&record.network)?;
    let client = reqwest::Client::new();
    let api_key = load_toncenter_api_key();
    let mut request = client
        .get(format!("{endpoint}/api/v2/getAddressBalance"))
        .query(&[("address", record.address.as_str())]);
    if let Some(api_key) = api_key.as_deref() {
        request = request.header("X-API-Key", api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Failed to load wallet balance: {error}"))?;
    let response = checked_toncenter_response(response, "load wallet balance")?;
    let envelope = response
        .json::<ToncenterEnvelope<String>>()
        .await
        .map_err(|error| format!("Failed to decode wallet balance: {error}"))?;
    if !envelope.ok {
        return Err(envelope
            .error
            .unwrap_or_else(|| "Failed to load wallet balance".to_owned()));
    }
    Ok(WalletBalanceResult {
        balance: envelope.result.unwrap_or_else(|| "0".to_owned()),
    })
}

#[tauri::command]
async fn get_wallet_activity(
    app: AppHandle,
    vault: State<'_, WalletVault>,
    request: WalletActivityRequest,
) -> Result<Vec<WalletActivityItem>, String> {
    let record = {
        let _guard = vault
            .operation_lock
            .lock()
            .map_err(|_| "Device vault lock is poisoned".to_owned())?;
        read_wallets(&wallets_path(&app)?)?
            .into_iter()
            .find(|wallet| wallet.id == request.wallet_id)
            .ok_or_else(|| "This wallet is not available on this device".to_owned())?
    };
    let endpoint = toncenter_endpoint(&record.network)?;
    let client = reqwest::Client::new();
    let api_key = load_toncenter_api_key();
    let limit = request.limit.unwrap_or(10).clamp(1, 25).to_string();
    let mut request = client
        .get(format!("{endpoint}/api/v2/getTransactions"))
        .query(&[
            ("address", record.address.as_str()),
            ("limit", limit.as_str()),
            ("archival", "true"),
        ]);
    if let Some(api_key) = api_key.as_deref() {
        request = request.header("X-API-Key", api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Failed to load wallet activity: {error}"))?;
    let response = checked_toncenter_response(response, "load wallet activity")?;
    let envelope = response
        .json::<ToncenterEnvelope<Vec<serde_json::Value>>>()
        .await
        .map_err(|error| format!("Failed to decode wallet activity: {error}"))?;
    if !envelope.ok {
        return Err(envelope
            .error
            .unwrap_or_else(|| "Failed to load wallet activity".to_owned()));
    }
    Ok(envelope
        .result
        .unwrap_or_default()
        .iter()
        .filter_map(parse_wallet_activity_item)
        .collect())
}

#[tauri::command]
async fn send_gram_transfer(
    app: AppHandle,
    vault: State<'_, WalletVault>,
    request: GramTransferRequest,
) -> Result<GramTransferResult, String> {
    let amount_nano = request
        .amount_nano
        .parse::<u128>()
        .map_err(|_| "Enter a valid GRAM amount".to_owned())?;
    if amount_nano == 0 {
        return Err("Enter an amount greater than zero".to_owned());
    }
    if request
        .comment
        .as_ref()
        .is_some_and(|comment| comment.len() > 120)
    {
        return Err("Comments are limited to 120 UTF-8 bytes".to_owned());
    }
    let recipient = TonAddress::from_str(request.recipient.trim())
        .map_err(|error| format!("Enter a valid recipient address: {error}"))?;

    let (record, mnemonic) = {
        let _guard = vault
            .operation_lock
            .lock()
            .map_err(|_| "Device vault lock is poisoned".to_owned())?;
        let wallets = read_wallets(&wallets_path(&app)?)?;
        let record = wallets
            .into_iter()
            .find(|wallet| wallet.id == request.wallet_id)
            .ok_or_else(|| "This wallet is not available on this device".to_owned())?;
        let mnemonic = Zeroizing::new(
            keyring_entry(&request.wallet_id)?
                .get_password()
                .map_err(|error| format!("Failed to access the recovery phrase: {error}"))?,
        );
        (record, mnemonic)
    };

    let version = match record.version.as_str() {
        "v4r2" => WalletVersion::V4R2,
        "v5r1" => WalletVersion::V5R1,
        _ => return Err("This wallet contract is not supported for transfers".to_owned()),
    };
    let wallet_id = match (version, record.network.as_str()) {
        (WalletVersion::V5R1, "testnet" | "mainnet") => WALLET_V5R1_ID_DEFAULT,
        (_, "testnet" | "mainnet") => WALLET_ID_DEFAULT,
        _ => return Err("This wallet network is not supported for transfers".to_owned()),
    };
    let key_pair = Mnemonic::from_str(mnemonic.as_str(), None)
        .and_then(|mnemonic| mnemonic.to_key_pair())
        .map_err(|error| format!("Failed to derive the wallet signing key: {error}"))?;
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)
        .map_err(|error| format!("Failed to initialize the wallet contract: {error}"))?;
    let stored_address = TonAddress::from_str(&record.address)
        .map_err(|error| format!("Stored wallet address is invalid: {error}"))?;
    if wallet.address != stored_address {
        return Err("The recovery phrase does not match this wallet address".to_owned());
    }

    let endpoint = toncenter_endpoint(&record.network)?;
    let client = reqwest::Client::new();
    let api_key = load_toncenter_api_key();
    let seqno = load_wallet_seqno(
        &client,
        endpoint,
        &record.address,
        api_key.as_ref().map(|key| key.as_str()),
    )
    .await?;
    let body = build_comment_body(request.comment.as_deref().unwrap_or_default())?;
    let message = Msg::new(
        CommonMsgInfoInt::new(recipient.to_msg_address(), TLBCoins::new(amount_nano)),
        body,
    )
    .to_cell()
    .map_err(|error| format!("Failed to encode the transfer message: {error}"))?;
    let valid_until = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("System clock is unavailable: {error}"))?
        .as_secs()
        .saturating_add(300)
        .try_into()
        .map_err(|_| "System clock is outside the supported range".to_owned())?;
    let external = wallet
        .create_ext_in_msg(vec![message], seqno, valid_until, seqno == 0)
        .map_err(|error| format!("Failed to sign the transfer: {error}"))?;
    let boc = STANDARD.encode(
        external
            .to_boc()
            .map_err(|error| format!("Failed to encode the signed transfer: {error}"))?,
    );

    send_boc(
        &client,
        endpoint,
        &boc,
        api_key.as_ref().map(|key| key.as_str()),
    )
    .await
}

fn build_comment_body(comment: &str) -> Result<TonCell, String> {
    let mut builder = TonCell::builder();
    0u32.write(&mut builder)
        .map_err(|error| format!("Failed to encode the comment opcode: {error}"))?;
    builder
        .write_bits(comment.as_bytes(), comment.len() * 8)
        .map_err(|error| format!("Failed to encode the comment: {error}"))?;
    builder
        .build()
        .map_err(|error| format!("Failed to build the comment cell: {error}"))
}

async fn load_wallet_seqno(
    client: &reqwest::Client,
    endpoint: &str,
    address: &str,
    api_key: Option<&str>,
) -> Result<u32, String> {
    let mut request = client
        .get(format!("{endpoint}/api/v2/getWalletInformation"))
        .query(&[("address", address)]);
    if let Some(api_key) = api_key {
        request = request.header("X-API-Key", api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Failed to load wallet state: {error}"))?;
    let response = checked_toncenter_response(response, "load wallet state")?;
    let envelope = response
        .json::<ToncenterEnvelope<serde_json::Value>>()
        .await
        .map_err(|error| format!("Failed to decode wallet state: {error}"))?;
    if !envelope.ok {
        return Err(envelope
            .error
            .unwrap_or_else(|| "Failed to load wallet state".to_owned()));
    }
    let Some(result) = envelope.result else {
        return Ok(0);
    };
    Ok(result
        .get("seqno")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32)
}

async fn send_boc(
    client: &reqwest::Client,
    endpoint: &str,
    boc: &str,
    api_key: Option<&str>,
) -> Result<GramTransferResult, String> {
    let mut request = client
        .post(format!("{endpoint}/api/v2/sendBocReturnHash"))
        .json(&serde_json::json!({"boc": boc}));
    if let Some(api_key) = api_key {
        request = request.header("X-API-Key", api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("Failed to submit the transfer: {error}"))?;
    let response = checked_toncenter_response(response, "submit the transfer")?;
    let envelope = response
        .json::<ToncenterEnvelope<serde_json::Value>>()
        .await
        .map_err(|error| format!("Failed to decode the network response: {error}"))?;
    if !envelope.ok {
        return Err(envelope
            .error
            .unwrap_or_else(|| "The network rejected the transfer".to_owned()));
    }
    let result = envelope
        .result
        .ok_or_else(|| "The network did not return a message hash".to_owned())?;
    let message_hash = result
        .get("hash")
        .and_then(serde_json::Value::as_str)
        .or_else(|| result.as_str())
        .ok_or_else(|| "The network did not return a message hash".to_owned())?
        .to_owned();
    Ok(GramTransferResult { message_hash })
}

fn wallets_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve the application data directory: {error}"))?;
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create the application data directory: {error}"))?;
    Ok(directory.join(WALLETS_FILE))
}

fn read_wallets(path: &Path) -> Result<Vec<WalletRecord>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let serialized = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read wallet metadata: {error}"))?;
    serde_json::from_str(&serialized)
        .map_err(|error| format!("Failed to parse wallet metadata: {error}"))
}

fn write_wallets(path: &Path, wallets: &[WalletRecord]) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(wallets)
        .map_err(|error| format!("Failed to encode wallet metadata: {error}"))?;
    let temporary_path = path.with_extension("json.tmp");
    fs::write(&temporary_path, serialized)
        .map_err(|error| format!("Failed to write wallet metadata: {error}"))?;
    fs::rename(&temporary_path, path)
        .map_err(|error| format!("Failed to replace wallet metadata: {error}"))
}

fn keyring_entry(wallet_id: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, wallet_id)
        .map_err(|error| format!("Failed to access the system keychain: {error}"))
}

fn parse_wallet_activity_item(value: &serde_json::Value) -> Option<WalletActivityItem> {
    let hash = value
        .pointer("/transaction_id/hash")
        .and_then(serde_json::Value::as_str)?
        .to_owned();
    let timestamp = value
        .get("utime")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    let fee_nano = json_string(value.get("fee")).unwrap_or_else(|| "0".to_owned());
    let in_message = value.get("in_msg");
    let out_messages = value
        .get("out_msgs")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    let (direction, value_nano, counterparty) = if out_messages.is_empty() {
        let value_nano = in_message
            .and_then(|message| json_string(message.get("value")))
            .unwrap_or_else(|| "0".to_owned());
        let source = in_message
            .and_then(|message| json_string(message.get("source")))
            .filter(|address| !address.is_empty());
        let direction = if value_nano.parse::<u128>().unwrap_or_default() > 0 {
            "incoming"
        } else {
            "contract"
        };
        (direction.to_owned(), value_nano, source)
    } else {
        let value_nano = out_messages
            .iter()
            .filter_map(|message| json_string(message.get("value")))
            .filter_map(|value| value.parse::<u128>().ok())
            .fold(0u128, u128::saturating_add)
            .to_string();
        let destination = out_messages
            .first()
            .and_then(|message| json_string(message.get("destination")))
            .filter(|address| !address.is_empty());
        ("outgoing".to_owned(), value_nano, destination)
    };

    Some(WalletActivityItem {
        hash,
        timestamp,
        direction,
        value_nano,
        fee_nano,
        counterparty,
    })
}

fn json_string(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn toncenter_endpoint(network: &str) -> Result<&'static str, String> {
    match network {
        "testnet" => Ok("https://testnet.toncenter.com"),
        "mainnet" => Ok("https://toncenter.com"),
        _ => Err("This wallet network is not supported".to_owned()),
    }
}

fn checked_toncenter_response(
    response: reqwest::Response,
    operation: &str,
) -> Result<reqwest::Response, String> {
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(
            "Toncenter rate limit reached. Check the API key and try again shortly.".to_owned(),
        );
    }
    response
        .error_for_status()
        .map_err(|error| format!("Failed to {operation}: {error}"))
}

fn load_toncenter_api_key() -> Option<Zeroizing<String>> {
    let entry = Entry::new(KEYRING_SERVICE, TONCENTER_KEYRING_ACCOUNT).ok()?;
    if let Ok(value) = entry.get_password() {
        let value = Zeroizing::new(value);
        if !value.trim().is_empty() {
            return Some(value);
        }
    }

    let dotenv_path = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.join(".env");
    let contents = Zeroizing::new(fs::read_to_string(dotenv_path).ok()?);
    let value = Zeroizing::new(parse_dotenv_value(&contents, TONCENTER_DOTENV_KEY)?);
    let _ = entry.set_password(&value);
    Some(value)
}

fn parse_dotenv_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let (name, value) = line.trim().split_once('=')?;
        if name.trim() != key {
            return None;
        }
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .or_else(|| {
                value
                    .strip_prefix('\'')
                    .and_then(|value| value.strip_suffix('\''))
            })
            .unwrap_or(value)
            .trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(WalletVault::default())
        .invoke_handler(tauri::generate_handler![
            list_wallets,
            save_wallet,
            get_wallet_balance,
            get_wallet_activity,
            send_gram_transfer,
            remove_wallet
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Acton Dev Wallet");
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALLETKIT_FIXTURE_MNEMONIC: &str = "dose ice enrich trigger test dove century still \
        betray gas diet dune use other base gym mad law immense village world example praise game";

    #[test]
    fn parses_quoted_dotenv_api_key_without_exposing_other_values() {
        let contents = "OTHER=value\nVITE_EXPLORER_TONCENTER_API_KEY=\"test-key\"\n";

        assert_eq!(
            parse_dotenv_value(contents, TONCENTER_DOTENV_KEY),
            Some("test-key".to_owned())
        );
    }

    #[test]
    fn v5r1_address_matches_walletkit_default_wallet_id() {
        let key_pair = Mnemonic::from_str(WALLETKIT_FIXTURE_MNEMONIC, None)
            .and_then(|mnemonic| mnemonic.to_key_pair())
            .expect("fixture mnemonic should derive");
        let wallet =
            TonWallet::new_with_params(WalletVersion::V5R1, key_pair, 0, WALLET_V5R1_ID_DEFAULT)
                .expect("fixture wallet should derive");

        let walletkit_testnet_address =
            TonAddress::from_str("0QDbuUzGbucJ6xkeLjr5O5s7A2u8xp-3DlxCbO8Lm3kwoclb")
                .expect("WalletKit fixture address should parse");

        assert_eq!(wallet.address, walletkit_testnet_address);
    }

    #[test]
    fn maps_toncenter_transactions_into_wallet_activity() {
        let incoming = serde_json::json!({
            "utime": 1_784_840_878u64,
            "transaction_id": {"hash": "incoming-hash"},
            "fee": "51868",
            "in_msg": {"source": "EQSender", "value": "2000000000"},
            "out_msgs": []
        });
        let outgoing = serde_json::json!({
            "utime": 1_784_842_816u64,
            "transaction_id": {"hash": "outgoing-hash"},
            "fee": "580094",
            "in_msg": {"source": "", "value": "0"},
            "out_msgs": [
                {"destination": "EQFirst", "value": "100000000"},
                {"destination": "EQSecond", "value": "250000000"}
            ]
        });

        assert_eq!(
            parse_wallet_activity_item(&incoming),
            Some(WalletActivityItem {
                hash: "incoming-hash".to_owned(),
                timestamp: 1_784_840_878,
                direction: "incoming".to_owned(),
                value_nano: "2000000000".to_owned(),
                fee_nano: "51868".to_owned(),
                counterparty: Some("EQSender".to_owned()),
            })
        );
        assert_eq!(
            parse_wallet_activity_item(&outgoing),
            Some(WalletActivityItem {
                hash: "outgoing-hash".to_owned(),
                timestamp: 1_784_842_816,
                direction: "outgoing".to_owned(),
                value_nano: "350000000".to_owned(),
                fee_nano: "580094".to_owned(),
                counterparty: Some("EQFirst".to_owned()),
            })
        );
    }
}
