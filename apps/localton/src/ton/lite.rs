use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use tonutils::{
    liteclient::{
        boc::{SimpleAccount, SimpleAccountState},
        client::LiteClient,
    },
    network_config::ConfigGlobal,
    tl::{
        common::{BlockId, BlockIdExt},
        response::TransactionId,
    },
    tlb::Account,
    tvm::Address,
};

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
    pub async fn connect(global_config: &Path) -> Result<Self> {
        let source = fs::read_to_string(global_config)
            .with_context(|| format!("failed to read global config {}", global_config.display()))?;
        let config: ConfigGlobal = source
            .parse()
            .with_context(|| format!("invalid global config {}", global_config.display()))?;
        let inner = LiteClient::connect_first(&config)
            .await
            .context("failed to connect to local liteserver")?;
        Ok(Self { inner })
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
        let bytes = self
            .inner
            .get_block(id.clone())
            .await
            .context("getBlock failed")?;
        Ok((id.into(), bytes))
    }

    pub async fn transactions(
        &mut self,
        workchain: i32,
        shard: &str,
        seqno: u32,
        count: u32,
    ) -> Result<(BlockRef, Vec<TransactionRef>, bool)> {
        let id = self.lookup(workchain, shard, seqno).await?;
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
    use super::parse_shard;

    #[test]
    fn parses_signed_and_hex_shards() {
        assert_eq!(parse_shard("-9223372036854775808").unwrap(), i64::MIN);
        assert_eq!(parse_shard("8000000000000000").unwrap(), i64::MIN);
        assert_eq!(parse_shard("0x4000000000000000").unwrap(), 1_i64 << 62);
    }
}
