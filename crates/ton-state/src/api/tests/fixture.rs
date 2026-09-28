use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use base64::{Engine, engine::general_purpose::STANDARD};
use http_body_util::BodyExt;
use rocksdb::DB;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, HashBytes, Lazy};
use rston::dict::Dict;
use rston::models::{
    Account, AccountState, BlockId, BlockRef, BlockchainConfig, CurrencyCollection,
    DepthBalanceInfo, IntAddr, KeyBlockRef, KeyMaxLt, LibDescr, McStateExtra, OptionalAccount,
    ShardAccount, ShardAccounts, ShardDescription, ShardHashes, ShardIdent, ShardStateUnsplit,
    StdAddr, ValidatorInfo,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::{Semaphore, watch};
use ton_node_db::{BlockIndex, StateStore};
use tower::ServiceExt;

use crate::api::{Api, get_method::run_get_method};

// Native TVM logging is global, including across separate test routers.
pub(crate) static NATIVE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A validator-format database with real account cells and synthetic block IDs.
/// Requests go through the production router, snapshot reader, and native TVM.
pub(crate) struct Fixture {
    _directory: tempfile::TempDir,
    pub(crate) store: StateStore,
    pub(crate) api: Api,
}

impl Fixture {
    pub(crate) fn new(state: AccountState, libraries: Dict<HashBytes, LibDescr>) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let snapshot = directory.path().join("snapshot");
        std::fs::create_dir(&snapshot)?;
        let wallet: Value =
            serde_json::from_str(include_str!("../get_method/fixtures/wallet-v5.json"))?;
        let address: StdAddr = wallet["address"].as_str().unwrap().parse()?;
        let mut balance = CurrencyCollection::new(109_388_167_086);
        balance
            .other
            .as_dict_mut()
            .set(42, rston::num::VarUint248::from(123_u32))?;
        let mut accounts = ShardAccounts::new();
        accounts.set(
            address.address,
            DepthBalanceInfo {
                split_depth: 0,
                balance: balance.clone(),
            },
            ShardAccount {
                account: Lazy::new(&OptionalAccount(Some(Account {
                    address: IntAddr::Std(address),
                    storage_stat: Default::default(),
                    last_trans_lt: 900,
                    balance,
                    state,
                })))?,
                last_trans_hash: HashBytes([9; 32]),
                last_trans_lt: 899,
            },
        )?;
        let shard = CellBuilder::build_from(ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            seqno: 12,
            gen_utime: 1_700_000_123,
            gen_lt: 123456,
            accounts: Lazy::new(&accounts)?,
            ..Default::default()
        })?;
        let shard_id = state_id(&shard)?;
        let config = BlockchainConfig {
            address: HashBytes::ZERO,
            params: rston::models::BlockchainConfigParams::from_raw(Boc::decode_base64(
                include_str!("../../../../ton-executor/src/default_config.boc64"),
            )?),
        };
        let description = ShardDescription {
            seqno: shard_id.seqno,
            reg_mc_seqno: 100,
            start_lt: 0,
            end_lt: 123456,
            root_hash: shard_id.root_hash,
            file_hash: shard_id.file_hash,
            before_split: false,
            before_merge: false,
            want_split: false,
            want_merge: false,
            nx_cc_updated: false,
            next_catchain_seqno: 0,
            next_validator_shard: shard_id.shard.prefix(),
            min_ref_mc_seqno: 0,
            gen_utime: 1_700_000_123,
            split_merge_at: None,
            fees_collected: CurrencyCollection::ZERO,
            funds_created: CurrencyCollection::ZERO,
        };
        let mut extra = McStateExtra {
            shards: ShardHashes::from_shards([(&shard_id.shard, &description)])?,
            config,
            validator_info: ValidatorInfo {
                validator_list_hash_short: 0,
                catchain_seqno: 0,
                nx_cc_updated: false,
            },
            prev_blocks: Default::default(),
            after_key_block: false,
            last_key_block: Some(previous_block(90)),
            block_create_stats: None,
            global_balance: CurrencyCollection::ZERO,
        };
        for seqno in 0..100 {
            extra.prev_blocks.set(
                seqno,
                KeyMaxLt {
                    has_key_block: seqno == 90,
                    max_end_lt: u64::from(seqno),
                },
                KeyBlockRef {
                    is_key_block: seqno == 90,
                    block_ref: previous_block(seqno),
                },
            )?;
        }
        let master = CellBuilder::build_from(ShardStateUnsplit {
            shard_ident: ShardIdent::MASTERCHAIN,
            seqno: 100,
            gen_utime: 1_700_000_124,
            gen_lt: 123457,
            libraries,
            custom: Some(Lazy::new(&extra)?),
            ..Default::default()
        })?;
        let master_id = state_id(&master)?;
        let cells = DB::open_default(snapshot.join("celldb"))?;
        let db_state = DB::open_default(snapshot.join("state"))?;
        for (id, root) in [(master_id, master), (shard_id, shard)] {
            let bare = bare_id(id);
            let mut boxed = constructor(
                "tonNode.blockIdExt workchain:int shard:long seqno:int root_hash:int256 file_hash:int256 = tonNode.BlockIdExt",
            );
            boxed.extend(&bare);
            let key = format!("desc{}", STANDARD.encode(Sha256::digest(boxed)));
            let mut record = constructor(
                "db.celldb.value block_id:tonNode.blockIdExt prev:int256 next:int256 root_hash:int256 = db.celldb.Value",
            );
            record.extend(bare);
            record.extend([0; 64]);
            record.extend(root.repr_hash().as_slice());
            cells.put(key, record)?;
            let mut record = (-1_i32).to_le_bytes().to_vec();
            record.extend(1_i32.to_le_bytes());
            record.extend(Boc::encode(&root));
            cells.put(root.repr_hash().as_slice(), record)?;
        }
        let key = constructor("db.state.key.shardClient = db.state.Key");
        let mut record =
            constructor("db.state.shardClient block:tonNode.blockIdExt = db.state.ShardClient");
        record.extend(bare_id(master_id));
        db_state.put(Sha256::digest(key), record)?;
        drop((cells, db_state));
        let store = StateStore::open(&snapshot, &directory.path().join("updates"), 10_000)?;
        let (_, state) = watch::channel(store.snapshot());
        let api = Api {
            state,
            zero_state: master_id,
            history: Arc::new(BlockIndex::open(&directory.path().join("history"))?),
            execution_slot: Arc::new(Semaphore::new(1)),
        };
        Ok(Self {
            _directory: directory,
            store,
            api,
        })
    }

    pub(crate) async fn request(&self, body: Value) -> Result<Value> {
        self.raw(&serde_json::to_vec(&body)?).await
    }

    pub(crate) async fn raw(&self, body: &[u8]) -> Result<Value> {
        self.raw_at("/api/runGetMethod", body).await
    }

    pub(crate) async fn raw_at(&self, path: &str, body: &[u8]) -> Result<Value> {
        let app = axum::Router::new()
            .route("/api/runGetMethod", axum::routing::post(run_get_method))
            .route(
                "/api/simulate",
                axum::routing::post(crate::api::simulate::simulate),
            )
            .with_state(self.api.clone());
        let response = app
            .oneshot(
                Request::post(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_vec()))?,
            )
            .await?;
        let status = response.status().as_u16();
        let value: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        Ok(json!({ "status": status, "body": value }))
    }
}

fn constructor(schema: &str) -> Vec<u8> {
    crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC)
        .checksum(schema.as_bytes())
        .to_le_bytes()
        .to_vec()
}

fn bare_id(id: BlockId) -> Vec<u8> {
    let mut bytes = id.shard.workchain().to_le_bytes().to_vec();
    bytes.extend(id.shard.prefix().to_le_bytes());
    bytes.extend(id.seqno.to_le_bytes());
    bytes.extend(id.root_hash.0);
    bytes.extend(id.file_hash.0);
    bytes
}

fn previous_block(seqno: u32) -> BlockRef {
    BlockRef {
        end_lt: u64::from(seqno),
        seqno,
        root_hash: HashBytes([seqno as u8; 32]),
        file_hash: HashBytes([1; 32]),
    }
}

fn state_id(root: &Cell) -> Result<BlockId> {
    let state: ShardStateUnsplit = root.parse()?;
    Ok(BlockId {
        shard: state.shard_ident,
        seqno: state.seqno,
        root_hash: *root.repr_hash(),
        file_hash: Boc::file_hash(Boc::encode(root)),
    })
}
