use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use ed25519_dalek::SigningKey;
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use ton::{
    ton_core::{cell::TonCell, traits::tlb::TLB, types::TonAddress},
    ton_wallet::{
        KeyPair as TonKeyPair, TonWallet, WalletV4ExtMsgBody, WalletV5ExtMsgBody,
        WalletVersion as TonWalletVersion,
    },
};
use tonutils::tvm::Address;
use tycho_types::{
    boc::{Boc, BocRepr},
    cell::{Cell, CellBuilder, HashBytes},
    models::{
        CurrencyCollection, IntAddr, OwnedRelaxedMessage, RelaxedIntMsgInfo, RelaxedMsgInfo,
        StateInit, StdAddr,
    },
};
use utoipa::ToSchema;

use crate::{
    cli::{WalletCommand, WalletVersion},
    storage::Layout,
    ton::lite::LocalLiteClient,
    ton::toolchain::Toolchain,
    ton::tools::{
        lite_client::{LiteTarget, RunMethodRequest},
        types::OperationContext,
    },
};

const REGISTRY_SCHEMA: u32 = 1;
const MAX_GRAMS_NANO: u128 = (1_u128 << 120) - 1;

/// Wallet versions that Localton can manage
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StoredWalletVersion {
    V1,
    V2,
    V3,
    V4r2,
    V5r1,
    HighloadV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletRecord {
    pub name: String,
    pub version: StoredWalletVersion,
    pub workchain: i32,
    pub wallet_id: u32,
    pub address: String,
    pub key_base: PathBuf,
    pub address_file: PathBuf,
    pub deploy_boc: Option<PathBuf>,
    pub genesis: bool,
    pub created_at: i64,
}

/// Public wallet data without private keys
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicWallet {
    /// Stable wallet name
    pub name: String,
    /// Wallet contract version
    pub version: StoredWalletVersion,
    /// Wallet workchain
    pub workchain: i32,
    /// Wallet subwallet identifier
    pub wallet_id: u32,
    /// User-friendly TON address
    pub address: String,
    /// `true` for the genesis wallet
    pub genesis: bool,
    /// Unix time when Localton created the wallet
    pub created_at: i64,
}

impl From<&WalletRecord> for PublicWallet {
    fn from(value: &WalletRecord) -> Self {
        Self {
            name: value.name.clone(),
            version: value.version,
            workchain: value.workchain,
            wallet_id: value.wallet_id,
            address: value.address.clone(),
            genesis: value.genesis,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalletRegistry {
    schema_version: u32,
    wallets: BTreeMap<String, WalletRecord>,
}

impl Default for WalletRegistry {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA,
            wallets: BTreeMap::new(),
        }
    }
}

pub async fn execute(command: WalletCommand) -> Result<()> {
    match command {
        WalletCommand::List { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let registry = load_registry(&toolchain.layout)?;
            let values: Vec<_> = registry.wallets.values().map(PublicWallet::from).collect();
            println!("{}", serde_json::to_string_pretty(&values)?);
        }
        WalletCommand::Create {
            state,
            name,
            version,
            workchain,
            wallet_id,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let wallet = create_wallet(&toolchain, &name, version, workchain, wallet_id).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&PublicWallet::from(&wallet))?
            );
        }
        WalletCommand::Send {
            state,
            from,
            to,
            amount,
            comment,
            body,
            state_init,
            mode,
            no_bounce,
        } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let status = send(
                &toolchain,
                SendRequest {
                    from: &from,
                    to: &to,
                    amount: &amount,
                    comment: comment.as_deref(),
                    body: body.as_deref(),
                    state_init: state_init.as_deref(),
                    mode,
                    bounce: !no_bounce,
                },
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "from": from,
                    "to": to,
                    "amount": amount,
                    "status": status,
                }))?
            );
        }
        WalletCommand::Fund {
            state,
            wallet,
            amount,
        } => {
            let funded = fund_wallet(&state.state_dir, &wallet, &amount).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "wallet": wallet,
                    "address": funded.address,
                    "amount": amount,
                    "status": funded.status,
                }))?
            );
        }
        WalletCommand::Info { state, wallet } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let registry = load_registry(&toolchain.layout)?;
            let address = resolve_wallet_or_address(&registry, &wallet)?;
            let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&client.account(&address).await?)?
            );
        }
    }
    Ok(())
}

pub fn load_public(layout: &Layout) -> Result<Vec<PublicWallet>> {
    Ok(load_registry(layout)?
        .wallets
        .values()
        .map(PublicWallet::from)
        .collect())
}

pub fn wallet(layout: &Layout, name: &str) -> Result<WalletRecord> {
    load_registry(layout)?
        .wallets
        .get(name)
        .cloned()
        .with_context(|| format!("unknown wallet `{name}`"))
}

pub async fn ensure_wallet(
    state_dir: &Path,
    name: &str,
    version: WalletVersion,
    workchain: i32,
    wallet_id: u32,
) -> Result<PublicWallet> {
    let toolchain = Toolchain::resolve(state_dir, None).await?;
    ensure_wallet_for_toolchain(&toolchain, name, version, workchain, wallet_id).await
}

pub(crate) async fn ensure_wallet_for_toolchain(
    toolchain: &Toolchain,
    name: &str,
    version: WalletVersion,
    workchain: i32,
    wallet_id: u32,
) -> Result<PublicWallet> {
    let registry = load_registry(&toolchain.layout)?;
    if let Some(existing) = registry.wallets.get(name) {
        ensure!(
            existing.workchain == workchain,
            "wallet `{name}` already exists in workchain {}, expected {workchain}",
            existing.workchain
        );
        return Ok(PublicWallet::from(existing));
    }
    let wallet = create_wallet(toolchain, name, version, workchain, wallet_id).await?;
    Ok(PublicWallet::from(&wallet))
}

pub struct FundWalletResult {
    pub address: String,
    pub status: u32,
}

pub async fn fund_wallet(state_dir: &Path, wallet: &str, amount: &str) -> Result<FundWalletResult> {
    let toolchain = Toolchain::resolve(state_dir, None).await?;
    let registry = load_registry(&toolchain.layout)?;
    let destination = resolve_wallet_or_address(&registry, wallet)?;
    let status = send(
        &toolchain,
        SendRequest {
            from: "faucet",
            to: &destination,
            amount,
            comment: None,
            body: None,
            state_init: None,
            mode: 3,
            bounce: false,
        },
    )
    .await?;
    if let Some(record) = registry.wallets.get(wallet)
        && let Some(deploy) = record.deploy_boc.as_ref()
    {
        wait_for_balance(&toolchain.layout.global_config, &record.address).await?;
        let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
        client
            .send_boc(
                fs::read(deploy).with_context(|| {
                    format!("failed to read deployment BoC {}", deploy.display())
                })?,
            )
            .await?;
    }
    Ok(FundWalletResult {
        address: destination,
        status,
    })
}

async fn create_wallet(
    toolchain: &Toolchain,
    name: &str,
    version: WalletVersion,
    workchain: i32,
    wallet_id: u32,
) -> Result<WalletRecord> {
    validate_name(name)?;
    let mut registry = load_registry(&toolchain.layout)?;
    ensure!(
        !registry.wallets.contains_key(name),
        "wallet `{name}` already exists"
    );
    let wallet_dir = toolchain.layout.wallets.join(name);
    fs::create_dir_all(&wallet_dir)?;
    fs::set_permissions(&wallet_dir, fs::Permissions::from_mode(0o700))?;
    let base = wallet_dir.join("wallet");

    let (stored_version, address_file, deploy_boc) = match version {
        WalletVersion::V1 => {
            run_fift(
                toolchain,
                &wallet_dir,
                "new-wallet.fif",
                vec![workchain.to_string(), path_text(&base)],
            )
            .await?;
            (
                StoredWalletVersion::V1,
                base.with_extension("addr"),
                Some(base.with_file_name("wallet-query.boc")),
            )
        }
        WalletVersion::V2 => {
            run_fift(
                toolchain,
                &wallet_dir,
                "new-wallet-v2.fif",
                vec![workchain.to_string(), path_text(&base)],
            )
            .await?;
            (
                StoredWalletVersion::V2,
                base.with_extension("addr"),
                Some(base.with_file_name("wallet-query.boc")),
            )
        }
        WalletVersion::V3 => {
            run_fift(
                toolchain,
                &wallet_dir,
                "new-wallet-v3.fif",
                vec![
                    workchain.to_string(),
                    wallet_id.to_string(),
                    path_text(&base),
                ],
            )
            .await?;
            (
                StoredWalletVersion::V3,
                base.with_extension("addr"),
                Some(base.with_file_name("wallet-query.boc")),
            )
        }
        WalletVersion::Highload => {
            run_fift(
                toolchain,
                &wallet_dir,
                "new-highload-wallet-v2.fif",
                vec![
                    workchain.to_string(),
                    wallet_id.to_string(),
                    path_text(&base),
                ],
            )
            .await?;
            (
                StoredWalletVersion::HighloadV2,
                PathBuf::from(format!("{}{wallet_id}.addr", base.display())),
                Some(PathBuf::from(format!(
                    "{}{wallet_id}-query.boc",
                    base.display()
                ))),
            )
        }
        version @ (WalletVersion::V4r2 | WalletVersion::V5r1) => {
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);
            let signing_key = SigningKey::from_bytes(&seed);
            write_private_key(&base.with_extension("pk"), &seed)?;
            let valid_until = unix_time_u32()?.saturating_add(3600);
            let (stored, ton_version) = match version {
                WalletVersion::V4r2 => (StoredWalletVersion::V4r2, TonWalletVersion::V4R2),
                WalletVersion::V5r1 => (StoredWalletVersion::V5r1, TonWalletVersion::V5R1),
                _ => unreachable!(),
            };
            let wallet = ton_wallet(ton_version, &signing_key, workchain, wallet_id)?;
            let address = wallet.address;
            let deploy = wallet
                .create_ext_in_msg(Vec::new(), 0, valid_until, true)?
                .to_boc()?;
            let address_file = base.with_extension("addr");
            write_address_file(&address_file, &address)?;
            let deploy_boc = base.with_file_name("wallet-query.boc");
            fs::write(&deploy_boc, deploy)?;
            (stored, address_file, Some(deploy_boc))
        }
    };

    let address = read_address_file(&address_file)?.to_raw();
    ensure_private_permissions(&base.with_extension("pk"))?;
    let record = WalletRecord {
        name: name.to_owned(),
        version: stored_version,
        workchain,
        wallet_id,
        address,
        key_base: base,
        address_file,
        deploy_boc,
        genesis: false,
        created_at: unix_time_i64(),
    };
    registry.wallets.insert(name.to_owned(), record.clone());
    save_registry(&toolchain.layout, &registry)?;
    Ok(record)
}

struct TransferBuild<'a> {
    destination: &'a str,
    amount: &'a str,
    amount_nano: u128,
    seqno: u32,
    comment: Option<&'a str>,
    body: Option<&'a Path>,
    state_init: Option<&'a Path>,
    mode: u8,
    bounce: bool,
}

pub struct SendRequest<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub amount: &'a str,
    pub comment: Option<&'a str>,
    pub body: Option<&'a Path>,
    pub state_init: Option<&'a Path>,
    pub mode: u8,
    pub bounce: bool,
}

pub async fn send(toolchain: &Toolchain, request: SendRequest<'_>) -> Result<u32> {
    let message = build_message(toolchain, request).await?;
    let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
    client.send_boc(message.boc).await
}

pub(crate) async fn send_confirmed(toolchain: &Toolchain, request: SendRequest<'_>) -> Result<u32> {
    let message = build_message(toolchain, request).await?;
    let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
    let status = client.send_boc(message.boc).await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        if wallet_seqno(toolchain, &message.source_address).await? > message.seqno {
            return Ok(status);
        }
        if tokio::time::Instant::now() >= deadline {
            bail!(
                "wallet transaction with seqno {} did not become visible within 30 seconds",
                message.seqno
            )
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[derive(Debug, Clone)]
pub struct FundAccountMessage {
    pub boc: Vec<u8>,
    pub source_address: String,
    pub destination_address: String,
    pub seqno: u32,
}

#[derive(Debug)]
pub enum FundAccountError {
    InvalidRequest(String),
    Infrastructure(anyhow::Error),
}

pub async fn build_fund_account_message(
    state_dir: &Path,
    address: &str,
    amount_nano: u128,
) -> std::result::Result<FundAccountMessage, FundAccountError> {
    if amount_nano == 0 {
        return Err(FundAccountError::InvalidRequest(
            "amount must be greater than zero".to_owned(),
        ));
    }
    if amount_nano > MAX_GRAMS_NANO {
        return Err(FundAccountError::InvalidRequest(
            "amount exceeds the TON Grams range".to_owned(),
        ));
    }
    let destination = Address::from_str(address)
        .map_err(|_| FundAccountError::InvalidRequest(format!("invalid TON address `{address}`")))?
        .to_raw();
    let toolchain = Toolchain::resolve(state_dir, None)
        .await
        .map_err(FundAccountError::Infrastructure)?;
    let amount = format_nano_grams(amount_nano);
    let message = build_message(
        &toolchain,
        SendRequest {
            from: "faucet",
            to: &destination,
            amount: &amount,
            comment: None,
            body: None,
            state_init: None,
            mode: 3,
            bounce: false,
        },
    )
    .await
    .map_err(FundAccountError::Infrastructure)?;
    Ok(FundAccountMessage {
        boc: message.boc,
        source_address: message.source_address,
        destination_address: destination,
        seqno: message.seqno,
    })
}

struct BuiltMessage {
    boc: Vec<u8>,
    source_address: String,
    seqno: u32,
}

async fn build_message(toolchain: &Toolchain, request: SendRequest<'_>) -> Result<BuiltMessage> {
    let SendRequest {
        from,
        to,
        amount,
        comment,
        body,
        state_init,
        mode,
        bounce,
    } = request;
    ensure!(
        comment.is_none() || body.is_none(),
        "--comment and --body are mutually exclusive"
    );
    let amount_nano = parse_grams(amount)?;
    let registry = load_registry(&toolchain.layout)?;
    let source = registry
        .wallets
        .get(from)
        .with_context(|| format!("unknown source wallet `{from}`"))?;
    let destination = resolve_wallet_or_address(&registry, to)?;
    let seqno = wallet_seqno(toolchain, &source.address).await?;
    let transfer = TransferBuild {
        destination: &destination,
        amount,
        amount_nano,
        seqno,
        comment,
        body,
        state_init,
        mode,
        bounce,
    };
    let boc = match source.version {
        StoredWalletVersion::V1 | StoredWalletVersion::V2 | StoredWalletVersion::V3 => {
            build_fift_transfer(toolchain, source, &transfer).await?
        }
        StoredWalletVersion::HighloadV2 => {
            build_highload_transfer(toolchain, source, &destination, amount, mode, bounce).await?
        }
        StoredWalletVersion::V4r2 | StoredWalletVersion::V5r1 => {
            build_native_transfer(source, &transfer)?
        }
    };
    Ok(BuiltMessage {
        boc,
        source_address: source.address.clone(),
        seqno,
    })
}

async fn build_fift_transfer(
    toolchain: &Toolchain,
    source: &WalletRecord,
    transfer: &TransferBuild<'_>,
) -> Result<Vec<u8>> {
    let output_base = source
        .key_base
        .parent()
        .context("wallet base has no parent")?
        .join(format!("send-{}-{}", unix_time_i64(), transfer.seqno));
    let mut args = Vec::new();
    if !transfer.bounce {
        args.push("-n".to_owned());
    }
    args.extend(["-m".to_owned(), transfer.mode.to_string()]);
    if let Some(comment) = transfer.comment {
        args.extend(["-C".to_owned(), comment.to_owned()]);
    }
    if let Some(body) = transfer.body {
        args.extend(["-B".to_owned(), path_text(body)]);
    }
    if let Some(state_init) = transfer.state_init {
        args.extend(["-I".to_owned(), path_text(state_init)]);
    }
    args.push(path_text(&source.key_base));
    args.push(transfer.destination.to_owned());
    let script = match source.version {
        StoredWalletVersion::V1 => {
            args.push(transfer.seqno.to_string());
            args.push(transfer.amount.to_owned());
            "wallet.fif"
        }
        StoredWalletVersion::V2 => {
            args.push(transfer.seqno.to_string());
            args.push(transfer.amount.to_owned());
            "wallet-v2.fif"
        }
        StoredWalletVersion::V3 => {
            args.push(source.wallet_id.to_string());
            args.push(transfer.seqno.to_string());
            args.push(transfer.amount.to_owned());
            "wallet-v3.fif"
        }
        _ => bail!("wallet version does not use the simple Fift transfer path"),
    };
    args.push(path_text(&output_base));
    run_fift(
        toolchain,
        source
            .key_base
            .parent()
            .context("wallet base has no parent")?,
        script,
        args,
    )
    .await?;
    let boc = output_base.with_extension("boc");
    fs::read(&boc).with_context(|| format!("failed to read {}", boc.display()))
}

async fn build_highload_transfer(
    toolchain: &Toolchain,
    source: &WalletRecord,
    destination: &str,
    amount: &str,
    mode: u8,
    bounce: bool,
) -> Result<Vec<u8>> {
    let wallet_dir = source
        .key_base
        .parent()
        .context("wallet base has no parent")?;
    let suffix = unix_time_i64();
    let orders = wallet_dir.join(format!("orders-{suffix}.fif"));
    fs::write(&orders, format!("SEND {destination} {amount}\n"))?;
    let output_base = wallet_dir.join(format!("send-{suffix}"));
    let mut args = Vec::new();
    if !bounce {
        args.push("-n".to_owned());
    }
    args.extend([
        "-m".to_owned(),
        mode.to_string(),
        path_text(&source.key_base),
        source.wallet_id.to_string(),
        path_text(&orders),
        path_text(&output_base),
    ]);
    run_fift(toolchain, wallet_dir, "highload-wallet-v2.fif", args).await?;
    fs::read(output_base.with_extension("boc")).context("failed to read highload transfer BoC")
}

fn build_native_transfer(source: &WalletRecord, transfer: &TransferBuild<'_>) -> Result<Vec<u8>> {
    let seed = read_private_key(&source.key_base.with_extension("pk"))?;
    let signing_key = SigningKey::from_bytes(&seed);
    let destination = TonAddress::from_str(transfer.destination)
        .with_context(|| format!("invalid destination address `{}`", transfer.destination))?;
    let destination = ton_address_to_std(&destination)?;
    let body = load_body(transfer.comment, transfer.body)?;
    let init = load_state_init(transfer.state_init)?;
    let message = OwnedRelaxedMessage {
        info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
            bounce: transfer.bounce,
            dst: IntAddr::Std(destination),
            value: CurrencyCollection::new(transfer.amount_nano),
            ..Default::default()
        }),
        init,
        body: body.into(),
        layout: None,
    };
    let internal = TonCell::from_boc(BocRepr::encode(message)?)?;
    let valid_until = unix_time_u32()?.saturating_add(60);
    let version = match source.version {
        StoredWalletVersion::V4r2 => TonWalletVersion::V4R2,
        StoredWalletVersion::V5r1 => TonWalletVersion::V5R1,
        _ => bail!("wallet version does not use the native transfer path"),
    };
    let wallet = ton_wallet(version, &signing_key, source.workchain, source.wallet_id)?;
    let wallet_id = ton_wallet_id(source.wallet_id);
    let signing_body = match version {
        TonWalletVersion::V4R2 => WalletV4ExtMsgBody {
            subwallet_id: wallet_id,
            valid_until,
            msg_seqno: transfer.seqno,
            opcode: 0,
            msgs_modes: vec![transfer.mode],
            msgs: vec![internal],
        }
        .to_cell()?,
        TonWalletVersion::V5R1 => WalletV5ExtMsgBody {
            wallet_id,
            valid_until,
            msg_seqno: transfer.seqno,
            msgs_modes: vec![transfer.mode],
            msgs: vec![internal],
        }
        .to_cell()?,
        _ => unreachable!(),
    };
    wallet
        .create_ext_in_msg_from_body(wallet.sign_ext_in_body(&signing_body)?, transfer.seqno == 0)?
        .to_boc()
        .map_err(Into::into)
}

fn ton_wallet(
    version: TonWalletVersion,
    signing_key: &SigningKey,
    workchain: i32,
    wallet_id: u32,
) -> Result<TonWallet> {
    i8::try_from(workchain).context("wallet workchain must fit in i8")?;
    TonWallet::new_with_params(
        version,
        TonKeyPair {
            public_key: signing_key.verifying_key().to_bytes(),
            secret_key: signing_key.to_keypair_bytes(),
        },
        workchain,
        ton_wallet_id(wallet_id),
    )
    .map_err(Into::into)
}

const fn ton_wallet_id(wallet_id: u32) -> i32 {
    i32::from_be_bytes(wallet_id.to_be_bytes())
}

fn ton_address_to_std(address: &TonAddress) -> Result<StdAddr> {
    let workchain = i8::try_from(address.workchain).context("address workchain must fit in i8")?;
    let hash = <[u8; 32]>::try_from(address.hash.as_slice())
        .context("TON address hash must contain 32 bytes")?;
    Ok(StdAddr::new(workchain, HashBytes(hash)))
}

fn load_body(comment: Option<&str>, body: Option<&Path>) -> Result<Cell> {
    if let Some(path) = body {
        return Boc::decode(
            fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
        )
        .map_err(Into::into);
    }
    let mut builder = CellBuilder::new();
    if let Some(comment) = comment {
        builder.store_u32(0)?;
        builder.store_raw(
            comment.as_bytes(),
            u16::try_from(comment.len().saturating_mul(8)).context("comment is too long")?,
        )?;
    }
    Ok(builder.build()?)
}

fn load_state_init(path: Option<&Path>) -> Result<Option<StateInit>> {
    path.map(|path| {
        Boc::decode(fs::read(path).with_context(|| format!("failed to read {}", path.display()))?)?
            .parse::<StateInit>()
            .map_err(Into::into)
    })
    .transpose()
}

async fn wallet_seqno(toolchain: &Toolchain, address: &str) -> Result<u32> {
    let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
    let account = client.account(address).await?;
    if account.state == "nonexist" || account.state == "uninit" {
        return Ok(0);
    }
    let seqno = toolchain
        .lite_client_tool
        .run_method(
            &OperationContext::new(Duration::from_secs(30)),
            &LiteTarget::new(&toolchain.layout.global_config).with_label("localton"),
            RunMethodRequest::new(address, "seqno", vec![])?,
        )
        .await?
        .first_u64()?;
    u32::try_from(seqno).context("wallet seqno exceeds u32")
}

async fn wait_for_balance(global_config: &Path, address: &str) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let mut client = LocalLiteClient::connect(global_config).await?;
        if client
            .account(address)
            .await?
            .balance_nano
            .parse::<u128>()
            .unwrap_or_default()
            > 0
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("wallet funding did not become visible within 30 seconds")
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn run_fift(
    toolchain: &Toolchain,
    current_dir: &Path,
    script: &str,
    args: Vec<String>,
) -> Result<crate::ton::tools::fift::FiftOutput> {
    toolchain
        .run_fift_script(
            current_dir,
            toolchain.smartcont_script(script),
            args.into_iter().map(Into::into).collect(),
            Duration::from_secs(60),
        )
        .await
}

fn load_registry(layout: &Layout) -> Result<WalletRegistry> {
    fs::create_dir_all(&layout.wallets)?;
    fs::set_permissions(&layout.wallets, fs::Permissions::from_mode(0o700))?;
    let path = registry_path(layout);
    let mut registry = if path.is_file() {
        let registry: WalletRegistry = serde_json::from_slice(&fs::read(&path)?)?;
        ensure!(
            registry.schema_version == REGISTRY_SCHEMA,
            "unsupported wallet registry schema {}",
            registry.schema_version
        );
        registry
    } else {
        WalletRegistry::default()
    };
    import_genesis_wallets(layout, &mut registry)?;
    save_registry(layout, &registry)?;
    Ok(registry)
}

fn save_registry(layout: &Layout, registry: &WalletRegistry) -> Result<()> {
    let path = registry_path(layout);
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(registry)?)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    fs::rename(&temporary, &path)?;
    Ok(())
}

fn import_genesis_wallets(layout: &Layout, registry: &mut WalletRegistry) -> Result<()> {
    let definitions = [
        ("main-wallet", StoredWalletVersion::V1, 0),
        ("faucet", StoredWalletVersion::V3, 42),
        ("validator", StoredWalletVersion::V3, 42),
        ("validator-1", StoredWalletVersion::V3, 42),
        ("validator-2", StoredWalletVersion::V3, 42),
        ("validator-3", StoredWalletVersion::V3, 42),
        ("validator-4", StoredWalletVersion::V3, 42),
        ("validator-5", StoredWalletVersion::V3, 42),
        ("validator-6", StoredWalletVersion::V3, 42),
        ("validator-7", StoredWalletVersion::V3, 42),
    ];
    for (name, version, wallet_id) in definitions {
        if registry.wallets.contains_key(name) {
            continue;
        }
        let base = layout.zerostate.join(name);
        let address_file = base.with_extension("addr");
        let key = base.with_extension("pk");
        if !address_file.is_file() || !key.is_file() {
            continue;
        }
        ensure_private_permissions(&key)?;
        let address = read_address_file(&address_file)?;
        registry.wallets.insert(
            name.to_owned(),
            WalletRecord {
                name: name.to_owned(),
                version,
                workchain: i32::from(address.workchain),
                wallet_id,
                address: address.to_raw(),
                key_base: base,
                address_file,
                deploy_boc: None,
                genesis: true,
                created_at: 0,
            },
        );
    }
    Ok(())
}

fn resolve_wallet_or_address(registry: &WalletRegistry, value: &str) -> Result<String> {
    if let Some(wallet) = registry.wallets.get(value) {
        return Ok(wallet.address.clone());
    }
    Ok(Address::from_str(value)
        .with_context(|| format!("unknown wallet or invalid TON address `{value}`"))?
        .to_raw())
}

fn read_address_file(path: &Path) -> Result<Address> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    ensure!(
        bytes.len() == 36,
        "address file {} must contain 36 bytes",
        path.display()
    );
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes[..32]);
    let workchain = i32::from_le_bytes(bytes[32..].try_into()?);
    let workchain = i8::try_from(workchain)
        .with_context(|| format!("unsupported workchain in {}", path.display()))?;
    Ok(Address::new(workchain, hash))
}

fn write_address_file(path: &Path, address: &TonAddress) -> Result<()> {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(address.hash.as_slice());
    bytes.extend_from_slice(&address.workchain.to_le_bytes());
    fs::write(path, bytes)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn write_private_key(path: &Path, key: &[u8; 32]) -> Result<()> {
    fs::write(path, key)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn read_private_key(path: &Path) -> Result<[u8; 32]> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("private key {} must contain 32 bytes", path.display()))
}

fn ensure_private_permissions(path: &Path) -> Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    if permissions.mode() & 0o077 != 0 {
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn parse_grams(value: &str) -> Result<u128> {
    let value = value.trim();
    ensure!(!value.is_empty(), "amount is empty");
    ensure!(!value.starts_with('-'), "amount must be non-negative");
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    ensure!(
        !whole.is_empty() && whole.bytes().all(|byte| byte.is_ascii_digit()),
        "invalid Gram amount `{value}`"
    );
    ensure!(
        fraction.bytes().all(|byte| byte.is_ascii_digit()) && fraction.len() <= 9,
        "Gram amount must have at most 9 decimal places"
    );
    let whole: u128 = whole.parse().context("Gram amount is too large")?;
    let mut fractional = fraction.to_owned();
    fractional.extend(std::iter::repeat_n('0', 9 - fractional.len()));
    let fractional: u128 = if fractional.is_empty() {
        0
    } else {
        fractional.parse()?
    };
    let amount = whole
        .checked_mul(1_000_000_000)
        .and_then(|value| value.checked_add(fractional))
        .context("Gram amount exceeds u128 nanotons")?;
    ensure!(
        amount <= MAX_GRAMS_NANO,
        "amount exceeds the TON Grams range"
    );
    Ok(amount)
}

pub(crate) fn format_nano_grams(value: u128) -> String {
    let whole = value / 1_000_000_000;
    let fraction = value % 1_000_000_000;
    if fraction == 0 {
        return whole.to_string();
    }

    format!("{whole}.{fraction:09}")
        .trim_end_matches('0')
        .to_owned()
}

fn validate_name(name: &str) -> Result<()> {
    ensure!(!name.is_empty(), "wallet name is empty");
    ensure!(
        name.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'),
        "wallet name may contain only ASCII letters, digits, `-`, and `_`"
    );
    Ok(())
}

fn registry_path(layout: &Layout) -> PathBuf {
    layout.wallets.join("registry.json")
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn unix_time_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn unix_time_u32() -> Result<u32> {
    u32::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_secs(),
    )
    .context("Unix timestamp exceeds u32")
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use ton::{ton_core::traits::tlb::TLB, ton_wallet::WalletVersion as TonWalletVersion};
    use tycho_types::boc::Boc;

    use super::{MAX_GRAMS_NANO, format_nano_grams, parse_grams, ton_wallet};

    #[test]
    fn native_wallet_deploy_bocs_use_canonical_cell_order() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);

        for version in [TonWalletVersion::V4R2, TonWalletVersion::V5R1] {
            let wallet = ton_wallet(version, &signing_key, 0, 698_983_191).unwrap();
            let boc = wallet
                .create_ext_in_msg(Vec::new(), 0, 2_000_000_000, true)
                .unwrap()
                .to_boc()
                .unwrap();

            Boc::decode(&boc).unwrap();
        }
    }

    #[test]
    fn parses_exact_gram_amounts() {
        assert_eq!(parse_grams("1").unwrap(), 1_000_000_000);
        assert_eq!(parse_grams("1.25").unwrap(), 1_250_000_000);
        assert_eq!(parse_grams("0.000000001").unwrap(), 1);
        assert!(parse_grams("0.0000000001").is_err());
    }

    #[test]
    fn formats_exact_nano_gram_amounts() {
        assert_eq!(format_nano_grams(1), "0.000000001");
        assert_eq!(format_nano_grams(1_250_000_000), "1.25");
        assert_eq!(format_nano_grams(10_000_000_000), "10");
    }

    #[test]
    fn round_trips_amounts_larger_than_u64() {
        let amount = u128::from(u64::MAX) + 1;
        assert_eq!(parse_grams(&format_nano_grams(amount)).unwrap(), amount);
        assert!(parse_grams(&format_nano_grams(MAX_GRAMS_NANO + 1)).is_err());
    }
}
