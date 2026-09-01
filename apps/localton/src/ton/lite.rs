use std::{net::Ipv4Addr, path::Path, time::Duration};

use anyhow::{Context, Result, anyhow};
use crc::{CRC_16_XMODEM, Crc};
use fastnum::I512;
use num_bigint::BigInt;
use serde::Serialize;
use ton::{block_tlb::TVMStack, ton_core::traits::tlb::TLB};
use tonutils::{
    liteclient::{
        boc::{SimpleAccount, SimpleAccountState},
        client::LiteClient,
    },
    network_config::{ConfigGlobal, ConfigLiteServer, ConfigPublicKey},
    tl::{
        common::{BlockId, BlockIdExt},
        response::TransactionId,
    },
    tlb::Account,
    tvm::Address,
};
use tycho_types::{
    boc::Boc,
    cell::{Cell, CellFamily, LoadCell},
    merkle::MerkleProof,
    models::{ShardStateUnsplit, config::BlockchainConfigParams},
};

use crate::ton::{global_config::GlobalConfig, tools::types::TonPublicKey};

/// Requests the result and proof material returned by official `runmethod`.
///
/// `mode.2` carries the result stack. The remaining bits retain the shard and
/// state proofs plus library extras, so this adapter can add local verification
/// later without changing its wire contract.
const RUN_METHOD_MODE: u32 = 0x17;

/// Computes the selector used by TON to dispatch a named get method.
///
/// TON deliberately uses CRC-16/XMODEM here. `tonutils::method_name_to_id`
/// currently uses CRC-16/IBM-SDLC, which produces a different selector and can
/// make the VM execute the contract fallback path instead of the requested
/// method. Keeping this small protocol rule next to the liteserver adapter makes
/// every Localton get-method call use the same ID as the official lite-client.
fn ton_method_id(name: &str) -> u64 {
    const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);
    u64::from(CRC16.checksum(name.as_bytes())) | 0x1_0000
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockRef {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionRef {
    pub account: Option<String>,
    pub lt: Option<u64>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountInfo {
    pub address: String,
    pub state: String,
    pub balance_nano: String,
    pub last_transaction_lt: Option<u64>,
    pub last_transaction_hash: Option<String>,
    pub block: BlockRef,
    pub shard_block: BlockRef,
}

pub struct LocalLiteClient {
    inner: LiteClient,
}

impl LocalLiteClient {
    /// Connects to the first reachable liteserver in configured priority order.
    ///
    /// Every endpoint has a bounded connection attempt so one unavailable server
    /// cannot prevent the remaining authenticated endpoints from being tried.
    pub async fn connect(global_config: &Path) -> Result<Self> {
        let config = GlobalConfig::load(global_config)?;
        Self::connect_config(tonutils_config(&config)).await
    }

    /// Connects directly to one host-local liteserver from its typed endpoint identity.
    ///
    /// The liteserver client only needs an address and authentication key, so this
    /// path neither reads nor rewrites the durable network configuration.
    pub async fn connect_node(port: u16, public_key: TonPublicKey) -> Result<Self> {
        let config = ConfigGlobal {
            liteservers: vec![tonutils_liteserver(Ipv4Addr::LOCALHOST, port, public_key)],
        };

        Self::connect_config(config).await
    }

    async fn connect_config(config: ConfigGlobal) -> Result<Self> {
        const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

        if config.liteservers.is_empty() {
            return Err(anyhow!("global config has no liteservers"));
        }

        let mut failures = Vec::with_capacity(config.liteservers.len());
        for liteserver in &config.liteservers {
            let endpoint = liteserver.socket_addr();
            match LiteClient::connect_with_timeout(
                endpoint,
                liteserver.public_key(),
                CONNECT_TIMEOUT,
            )
            .await
            {
                Ok(inner) => return Ok(Self { inner }),
                Err(error) => failures.push(format!("{endpoint}: {error}")),
            }
        }

        Err(anyhow!(
            "failed to connect to any configured liteserver: {}",
            failures.join("; ")
        ))
    }

    pub async fn last(&mut self) -> Result<BlockRef> {
        let info = self
            .inner
            .get_masterchain_info()
            .await
            .context("getMasterchainInfo failed")?;
        Ok(info.last.into())
    }

    pub async fn account(&mut self, address: &str) -> Result<AccountInfo> {
        let parsed = Address::from_str(address)
            .with_context(|| format!("invalid TON address `{address}`"))?;
        let account = self
            .inner
            .get_account_state_simple(parsed)
            .await
            .with_context(|| format!("getAccountState failed for {address}"))?;
        Ok(account_info(address, account))
    }

    pub async fn block(
        &mut self,
        workchain: i32,
        shard: &str,
        seqno: u32,
    ) -> Result<(BlockRef, Vec<u8>)> {
        let id = self.lookup(workchain, shard, seqno).await?;
        self.download_block(id).await
    }

    pub async fn transactions(
        &mut self,
        workchain: i32,
        shard: &str,
        seqno: u32,
        count: u32,
    ) -> Result<(BlockRef, Vec<TransactionRef>, bool)> {
        let id = self.lookup(workchain, shard, seqno).await?;
        // `ShardStateUnsplit` validates its complete structural envelope while
        // leaving large dictionaries lazy. Ask the liteserver to retain every
        // state/config root needed for that parse; otherwise those references are
        // pruned exotic cells even though the four requested config values exist.
        let response = self
            .inner
            .list_block_transactions(id, count, None, false, false)
            .await
            .context("listBlockTransactions failed")?;
        Ok((
            response.id.into(),
            response.ids.into_iter().map(Into::into).collect(),
            response.incomplete,
        ))
    }

    pub async fn send_boc(&mut self, body: Vec<u8>) -> Result<u32> {
        self.inner
            .send_message(body)
            .await
            .context("sendMessage failed")
    }

    /// Runs one read-only get method against the latest masterchain state.
    ///
    /// Inputs are integers because Localton's wallet and elector workflows only
    /// need integer stack arguments. Keeping the stack typed prevents them from
    /// constructing release-specific lite-client command strings.
    pub async fn run_method(
        &mut self,
        address: &str,
        method: &str,
        arguments: Vec<BigInt>,
    ) -> Result<TVMStack> {
        let account = Address::from_str(address)
            .with_context(|| format!("invalid TON address `{address}`"))?;
        let block = self
            .inner
            .get_masterchain_info()
            .await
            .context("getMasterchainInfo failed before run method")?
            .last;
        let mut stack = TVMStack::default();
        let max_exclusive: BigInt = BigInt::from(1_u8) << 256_usize;
        let min_inclusive = -max_exclusive.clone();
        for argument in arguments {
            anyhow::ensure!(
                argument >= min_inclusive && argument < max_exclusive,
                "get method argument does not fit into a signed TVM int257"
            );
            stack.push_int(I512::parse_str(&argument.to_string()));
        }
        let result = self
            .inner
            .run_smc_method(
                RUN_METHOD_MODE,
                block,
                account,
                ton_method_id(method),
                stack
                    .to_boc()
                    .context("failed to serialize canonical TVM argument stack")?,
            )
            .await
            .with_context(|| format!("run get method `{method}` failed for {address}"))?;
        anyhow::ensure!(
            result.exit_code == 0,
            "run get method `{method}` exited with code {}",
            result.exit_code
        );
        TVMStack::from_boc(
            result
                .result
                .context("liteserver omitted the requested TVM result stack")?,
        )
        .context("liteserver returned an invalid canonical TVM result stack")
    }

    /// Reads selected on-chain configuration parameters at the latest block.
    ///
    /// The response contains a Merkle proof of the masterchain state, not a bare
    /// config dictionary. We virtualize that proof and load `McStateExtra.config`
    /// through canonical TON types so application code never guesses at proof-cell
    /// references or relies on the presentation format of the official CLI.
    pub async fn config_params(&mut self, params: Vec<i32>) -> Result<BlockchainConfigParams> {
        let block = self
            .inner
            .get_masterchain_info()
            .await
            .context("getMasterchainInfo failed before config query")?
            .last;
        let response = self
            .inner
            .get_config_params(
                block, params, true, true, true, true, true, true, true, true, true, true, false,
            )
            .await
            .context("getConfigParams failed")?;
        let proof_root = Boc::decode(&response.config_proof)
            .context("getConfigParams returned an invalid config proof BoC")?;
        let proof = MerkleProof::load_from_cell(proof_root.as_ref())
            .context("getConfigParams config proof is not a Merkle proof")?;
        let state_root = Cell::virtualize(proof.cell);
        let state = state_root.parse::<ShardStateUnsplit>().with_context(|| {
            format!(
                "config proof does not contain a masterchain shard state (root type: {:?})",
                state_root.cell_type()
            )
        })?;
        let extra = state
            .custom
            .context("config proof masterchain state has no McStateExtra")?
            .load()
            .context("config proof contains invalid McStateExtra")?;
        Ok(extra.config.params)
    }

    async fn lookup(&mut self, workchain: i32, shard: &str, seqno: u32) -> Result<BlockIdExt> {
        let shard = parse_shard(shard)?;
        let seqno = i32::try_from(seqno).context("block seqno exceeds i32")?;
        let header = self
            .inner
            .lookup_block(
                (),
                BlockId {
                    workchain,
                    shard,
                    seqno,
                },
                Some(()),
                None,
                None,
                false,
                false,
                false,
                false,
                false,
            )
            .await
            .context("lookupBlock failed")?;
        Ok(header.id)
    }

    async fn download_block(&mut self, id: BlockIdExt) -> Result<(BlockRef, Vec<u8>)> {
        let bytes = self
            .inner
            .get_block(id.clone())
            .await
            .context("getBlock failed")?;
        Ok((id.into(), bytes))
    }
}

/// Converts Localton's complete network config to the subset consumed by tonutils.
fn tonutils_config(config: &GlobalConfig) -> ConfigGlobal {
    ConfigGlobal {
        liteservers: config
            .liteserver_endpoints()
            .map(|(ip, port, public_key)| tonutils_liteserver(ip, port, public_key))
            .collect(),
    }
}

fn tonutils_liteserver(ip: Ipv4Addr, port: u16, public_key: TonPublicKey) -> ConfigLiteServer {
    ConfigLiteServer {
        ip: i32::from_be_bytes(ip.octets()).into(),
        port,
        id: ConfigPublicKey::Ed25519 {
            key: *public_key.as_bytes(),
        },
    }
}

pub fn parse_shard(value: &str) -> Result<i64> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        let raw = u64::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hexadecimal shard `{value}`"))?;
        return Ok(raw as i64);
    }
    if value.len() == 16 && !value.starts_with('-') && value.chars().all(|c| c.is_ascii_hexdigit())
        || value
            .chars()
            .any(|c| c.is_ascii_hexdigit() && c.is_ascii_alphabetic())
    {
        let raw = u64::from_str_radix(value, 16)
            .with_context(|| format!("invalid hexadecimal shard `{value}`"))?;
        return Ok(raw as i64);
    }
    value
        .parse::<i64>()
        .with_context(|| format!("invalid shard `{value}`"))
}

impl From<BlockIdExt> for BlockRef {
    fn from(value: BlockIdExt) -> Self {
        Self {
            workchain: value.workchain,
            shard: format!("{:016x}", value.shard as u64),
            seqno: u32::try_from(value.seqno).unwrap_or_default(),
            root_hash: value.root_hash.to_hex(),
            file_hash: value.file_hash.to_hex(),
        }
    }
}

impl From<TransactionId> for TransactionRef {
    fn from(value: TransactionId) -> Self {
        Self {
            account: value.account.map(|value| value.to_hex()),
            lt: value.lt,
            hash: value.hash.map(|value| value.to_hex()),
        }
    }
}

fn account_info(address: &str, account: SimpleAccount) -> AccountInfo {
    let state = match account.state {
        SimpleAccountState::None => "nonexist",
        SimpleAccountState::Uninit => "uninit",
        SimpleAccountState::Frozen => "frozen",
        SimpleAccountState::Active => "active",
    };
    let balance_nano = match account.account.as_ref() {
        Some(Account::Full { storage, .. }) => storage.balance.grams.0.to_string(),
        Some(Account::None) | None => "0".to_owned(),
    };
    AccountInfo {
        address: address.to_owned(),
        state: state.to_owned(),
        balance_nano,
        last_transaction_lt: account.last_transaction_lt,
        last_transaction_hash: account.last_transaction_hash.map(hex::encode),
        block: account.block_id.into(),
        shard_block: account.shard_block_id.into(),
    }
}

pub fn require_existing_config(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        Err(anyhow!(
            "global config does not exist at {}; start the network first",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};

    use tonutils::network_config::ConfigGlobal;

    use super::{LocalLiteClient, parse_shard, ton_method_id, tonutils_liteserver};
    use crate::ton::tools::types::TonPublicKey;

    #[tokio::test]
    async fn connection_failover_attempts_every_configured_liteserver() {
        let first = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let first_port = first.local_addr().unwrap().port();
        let second = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let second_port = second.local_addr().unwrap().port();
        drop((first, second));

        let config = ConfigGlobal {
            liteservers: vec![
                tonutils_liteserver(
                    Ipv4Addr::LOCALHOST,
                    first_port,
                    TonPublicKey::from_bytes([1; 32]),
                ),
                tonutils_liteserver(
                    Ipv4Addr::LOCALHOST,
                    second_port,
                    TonPublicKey::from_bytes([2; 32]),
                ),
            ],
        };
        let error = LocalLiteClient::connect_config(config)
            .await
            .err()
            .expect("both unavailable liteservers must fail");
        let message = error.to_string();

        assert!(message.contains(&format!("127.0.0.1:{first_port}")));
        assert!(message.contains(&format!("127.0.0.1:{second_port}")));
    }

    #[test]
    fn parses_signed_and_hex_shards() {
        assert_eq!(parse_shard("-9223372036854775808").unwrap(), i64::MIN);
        assert_eq!(parse_shard("8000000000000000").unwrap(), i64::MIN);
        assert_eq!(parse_shard("0x4000000000000000").unwrap(), 1_i64 << 62);
    }

    #[test]
    fn get_method_ids_match_official_ton_crc16() {
        assert_eq!(ton_method_id("seqno"), 85_143);
        assert_eq!(ton_method_id("active_election_id"), 86_535);
    }

    #[test]
    fn tonutils_liteserver_preserves_typed_endpoint() {
        let config = tonutils_liteserver(
            Ipv4Addr::new(192, 168, 27, 4),
            18_004,
            TonPublicKey::from_bytes([7; 32]),
        );

        assert_eq!(
            config.socket_addr(),
            SocketAddrV4::new(Ipv4Addr::new(192, 168, 27, 4), 18_004)
        );
        assert_eq!(config.public_key(), [7; 32]);
    }
}
