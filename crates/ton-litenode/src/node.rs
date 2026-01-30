use crate::executor::{ExecContext, TvmExecutor};
use crate::storage::{
    AccountDelta, AccountMeta, AccountStatus, BlockMeta, CellStore, Globals, History, Indexes,
    LatestState, MessagePool, MsgMeta, PendingCommit, ReverseLtKey, TxMeta,
};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use anyhow::Context;
use core::cmp;
use std::time::{SystemTime, UNIX_EPOCH};
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;
use tycho_types::models::{AccountState, Message, MsgInfo, ShardAccount};

pub struct Node {
    pub cas: CellStore,
    pub latest: LatestState,
    pub history: History,
    pub indexes: Indexes,
    pub globals: Globals,
    pub pool: MessagePool,
    pub executor: Box<dyn TvmExecutor>,
}

impl Node {
    pub fn new(executor: Box<dyn TvmExecutor>, config_boc: BocBytes) -> anyhow::Result<Self> {
        let config_hash = compute_boc_hash(&config_boc)?;
        let mut cas = CellStore::new();
        cas.put(config_boc, config_hash);

        Ok(Self {
            cas,
            latest: LatestState::new(),
            history: History::new(),
            indexes: Indexes::new(),
            globals: Globals::new(config_hash),
            pool: MessagePool::new(),
            executor,
        })
    }

    pub fn send_boc(&mut self, boc: BocBytes) -> anyhow::Result<(Hash256, Seqno, Vec<Hash256>)> {
        // 1. Validate & Store
        let hash = compute_boc_hash(&boc)?;
        tracing::info!(
            "send_boc: msg_hash={}, current_queue={}",
            hash.to_hex(),
            self.pool.external.len() + self.pool.internal.len()
        );
        if self.cas.get(&hash).is_none() {
            self.cas.put(boc.clone(), hash);
        }

        // 2. Parse minimal meta
        let msg_meta = parse_msg_meta(&boc, hash)?;

        // 3. Register MsgMeta
        self.history.msg_by_hash.insert(hash, msg_meta);

        // 4. Enqueue
        self.pool.push_external(hash);

        // 5. Mine one
        let (block_meta, tx_meta) = self.mine_one()?;

        Ok((tx_meta.tx_hash, block_meta.seqno, tx_meta.out_msg_hashes))
    }

    pub fn mine_one(&mut self) -> anyhow::Result<(BlockMeta, TxMeta)> {
        // 1. Select message
        let queue_size = self.pool.external.len() + self.pool.internal.len();
        tracing::debug!("Message pool size: {}", queue_size);

        let msg_hash = self
            .pool
            .pop_next(self.globals.queue_policy, &self.history.msg_by_hash)
            .context("Queue empty")?;

        // 2. Load inbound message
        let msg_meta = self
            .history
            .msg_by_hash
            .get(&msg_hash)
            .context("Msg meta missing")?;
        let msg_boc = self
            .cas
            .get(&msg_meta.msg_boc_hash)
            .context("Msg BOC missing")?;
        let dst = msg_meta
            .dst
            .ok_or_else(|| anyhow::anyhow!("Msg has no dst"))?;

        // 3. Load old account
        let old_meta = self.latest.accounts.get(&dst).cloned();
        let old_account_boc = if let Some(meta) = &old_meta {
            self.cas.get(&meta.account_hash).cloned()
        } else {
            None
        };

        // 4. Allocate LT & time
        let lt = self.globals.global_lt + self.globals.lt_step;
        self.globals.global_lt = lt;
        let gen_utime = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;

        // 5. Execute
        let config_boc = self
            .cas
            .get(&self.globals.config_boc_hash)
            .context("Config missing")?
            .clone();
        let ctx = ExecContext {
            lt,
            gen_utime,
            rand_seed: None,
        };

        let exec_result =
            self.executor
                .execute(old_account_boc.as_ref(), msg_boc, &ctx, &config_boc)?;

        // 6. Store outputs & 7. Derive hashes
        let tx_hash = compute_boc_hash(&exec_result.tx_boc)?;
        self.cas.put(exec_result.tx_boc.clone(), tx_hash);

        let mut balance_cache = None;
        let mut status = AccountStatus::Nonexist;
        let mut code_hash = None;
        let mut data_hash = None;

        let new_account_hash = if let Some(acc_boc) = &exec_result.new_account_boc {
            let h = compute_boc_hash(acc_boc)?;
            self.cas.put(acc_boc.clone(), h);

            // Parse for meta
            if let Ok(cell) = Boc::decode(acc_boc)
                && let Ok(sa) = cell.parse::<ShardAccount>()
                && let Ok(opt_acc) = sa.account.load()
                && let Some(acc) = opt_acc.0
            {
                balance_cache = Some(acc.balance.tokens.into());
                status = match acc.state {
                    AccountState::Uninit => AccountStatus::Uninit,
                    AccountState::Active(state) => {
                        if let Some(cell) = state.code {
                            let ch = Hash256(*cell.repr_hash().as_array());
                            let boc = Boc::encode(cell);
                            self.cas.put(boc, ch);
                            code_hash = Some(ch);
                        }
                        if let Some(cell) = state.data {
                            let dh = Hash256(*cell.repr_hash().as_array());
                            let boc = Boc::encode(cell);
                            self.cas.put(boc, dh);
                            data_hash = Some(dh);
                        }
                        AccountStatus::Active
                    }
                    AccountState::Frozen(_) => AccountStatus::Frozen,
                };
            }
            Some(h)
        } else {
            None
        };

        let mut out_msg_hashes = Vec::new();
        for out_boc in exec_result.out_msgs_boc {
            let h = compute_boc_hash(&out_boc)?;
            self.cas.put(out_boc.clone(), h);
            out_msg_hashes.push(h);

            let out_meta = parse_msg_meta(&out_boc, h)?;
            self.history.msg_by_hash.insert(h, out_meta);
        }

        // 8. Build dev block
        let seqno = self.globals.head_seqno + 1;
        let block_boc = create_dev_block_boc(seqno, tx_hash)?;
        let block_hash = compute_boc_hash(&block_boc)?;
        self.cas.put(block_boc, block_hash);

        let block_meta = BlockMeta {
            seqno,
            prev_seqno: if seqno > 1 { Some(seqno - 1) } else { None },
            gen_utime,
            start_lt: lt,
            end_lt: lt,
            tx_hash,
            block_boc_hash: block_hash,
        };

        let tx_meta = TxMeta {
            tx_hash,
            tx_boc_hash: tx_hash,
            account: dst,
            lt,
            now: gen_utime,
            success: true, // TODO: parse from tx
            total_fees: None,
            in_msg_hash: Some(msg_hash),
            out_msg_hashes: out_msg_hashes.clone(),
            block_seqno: seqno,
        };

        // 9. Prepare deltas
        let new_meta = new_account_hash.map(|hash| AccountMeta {
            account_hash: hash,
            status,
            balance_cache,
            last_trans_lt: Some(lt),
            last_trans_hash: Some(tx_hash),
            code_hash,
            data_hash,
        });

        let delta = AccountDelta {
            addr: dst,
            old_hash: old_meta.as_ref().map(|m| m.account_hash),
            new_hash: new_account_hash,
            old_meta,
            new_meta,
        };

        // 10. Commit
        let pending = PendingCommit {
            block_meta: block_meta.clone(),
            tx_meta: tx_meta.clone(),
            delta,
            out_msg_hashes,
            msg_to_tx: vec![(msg_hash, tx_hash)],
        };

        self.apply_commit(pending)?;

        Ok((block_meta, tx_meta))
    }

    fn apply_commit(&mut self, pending: PendingCommit) -> anyhow::Result<()> {
        tracing::info!(
            "Applying block commit: seqno={}, tx_hash={}",
            pending.block_meta.seqno,
            pending.tx_meta.tx_hash.to_hex()
        );
        // Apply delta
        if let Some(new_meta) = &pending.delta.new_meta {
            self.latest
                .accounts
                .insert(pending.delta.addr, new_meta.clone());
        } else {
            self.latest.accounts.remove(&pending.delta.addr);
        }

        // History
        self.history.blocks.push(pending.block_meta.clone());

        let seqno = pending.block_meta.seqno;
        if self.history.deltas_by_seqno.len() < seqno as usize {
            self.history
                .deltas_by_seqno
                .resize(seqno as usize, Vec::new());
        }
        // seqno is 1-based, index is seqno-1
        if seqno > 0 {
            self.history.deltas_by_seqno[seqno as usize - 1].push(pending.delta);
        }

        self.history
            .tx_by_hash
            .insert(pending.tx_meta.tx_hash, pending.tx_meta.clone());

        for (msg, tx) in pending.msg_to_tx {
            self.history.msg_to_tx.insert(msg, tx);
        }

        // Indexes
        let key = ReverseLtKey(cmp::Reverse(pending.tx_meta.lt), pending.tx_meta.tx_hash);
        self.indexes
            .tx_by_account
            .entry(pending.tx_meta.account)
            .or_default()
            .insert(key, pending.tx_meta.tx_hash);
        self.indexes
            .tx_by_block
            .insert(seqno, pending.tx_meta.tx_hash);

        // Enqueue out msgs
        for h in pending.out_msg_hashes {
            self.pool.push_internal(h);
        }

        let remaining = self.pool.external.len() + self.pool.internal.len();
        if remaining > 0 {
            tracing::info!("Queue size after commit: {} messages remaining", remaining);
        }

        self.globals.head_seqno = seqno;

        Ok(())
    }

    // --- Query API ---

    pub fn get_address_information(&self, addr: &Addr) -> Option<AccountMeta> {
        self.latest.accounts.get(addr).cloned()
    }

    pub fn get_address_information_at_block(
        &self,
        addr: &Addr,
        seqno: Seqno,
    ) -> Option<AccountMeta> {
        if seqno >= self.globals.head_seqno {
            return self.get_address_information(addr);
        }

        // Search backwards from seqno to find the state as it was after block 'seqno'
        for s in (1..=seqno).rev() {
            if s as usize > self.history.deltas_by_seqno.len() {
                continue;
            }
            let deltas = &self.history.deltas_by_seqno[s as usize - 1];
            for delta in deltas {
                if delta.addr == *addr {
                    return delta.new_meta.clone();
                }
            }
        }
        None
    }

    pub fn get_transactions(
        &self,
        addr: &Addr,
        limit: usize,
        lt: Option<Lt>,
        hash: Option<Hash256>,
    ) -> Vec<TxMeta> {
        let index = if let Some(map) = self.indexes.tx_by_account.get(addr) {
            map
        } else {
            return Vec::new();
        };

        let start_key = if let (Some(l), Some(h)) = (lt, hash) {
            ReverseLtKey(cmp::Reverse(l), h)
        } else if let Some(l) = lt {
            ReverseLtKey(cmp::Reverse(l), Hash256([0; 32]))
        } else {
            ReverseLtKey(cmp::Reverse(u64::MAX), Hash256([0; 32]))
        };

        index
            .range(start_key..)
            .take(limit)
            .filter_map(|(_, tx_hash)| self.history.tx_by_hash.get(tx_hash).cloned())
            .collect()
    }

    pub fn get_block_header(&self, seqno: Seqno) -> Option<BlockMeta> {
        if seqno == 0 || seqno as usize > self.history.blocks.len() {
            None
        } else {
            Some(self.history.blocks[seqno as usize - 1].clone())
        }
    }

    pub fn get_block_transactions_ext(&self, seqno: Seqno) -> Option<Vec<TxMeta>> {
        let tx_hash = self.indexes.tx_by_block.get(&seqno)?;
        let tx = self.history.tx_by_hash.get(tx_hash).cloned()?;
        Some(vec![tx])
    }

    pub fn get_message_meta(&self, hash: &Hash256) -> Option<(MsgMeta, BocBytes)> {
        let meta = self.history.msg_by_hash.get(hash).cloned()?;
        let boc = self.cas.get(&meta.msg_boc_hash).cloned()?;
        Some((meta, boc))
    }

    pub fn get_transactions_extended(
        &self,
        addr: &Addr,
        limit: usize,
        lt: Option<Lt>,
        hash: Option<Hash256>,
    ) -> Vec<(
        TxMeta,
        Option<(MsgMeta, BocBytes)>,
        Vec<(MsgMeta, BocBytes)>,
    )> {
        let txs = self.get_transactions(addr, limit, lt, hash);
        txs.into_iter()
            .map(|tx| {
                let in_msg = tx.in_msg_hash.and_then(|h| self.get_message_meta(&h));
                let out_msgs = tx
                    .out_msg_hashes
                    .iter()
                    .filter_map(|h| self.get_message_meta(h))
                    .collect();
                (tx, in_msg, out_msgs)
            })
            .collect()
    }

    pub fn get_transaction_by_hash_extended(
        &self,
        hash: &Hash256,
    ) -> Option<(
        TxMeta,
        Option<(MsgMeta, BocBytes)>,
        Vec<(MsgMeta, BocBytes)>,
    )> {
        let tx = self.history.tx_by_hash.get(hash).cloned()?;
        let in_msg = tx.in_msg_hash.and_then(|h| self.get_message_meta(&h));
        let out_msgs = tx
            .out_msg_hashes
            .iter()
            .filter_map(|h| self.get_message_meta(h))
            .collect();
        Some((tx, in_msg, out_msgs))
    }
}

// Helpers

fn compute_boc_hash(boc: &[u8]) -> anyhow::Result<Hash256> {
    let cell = Boc::decode(boc)?;
    let hash = cell.repr_hash();
    Ok(Hash256(*hash.as_array()))
}

fn parse_msg_meta(boc: &[u8], hash: Hash256) -> anyhow::Result<MsgMeta> {
    let cell = Boc::decode(boc)?;
    let msg = cell.parse::<Message<'_>>()?;

    let (src, dst, value, bounce) = match msg.info {
        MsgInfo::Int(info) => (
            Some(convert_addr(&info.src)),
            Some(convert_addr(&info.dst)),
            Some(info.value.tokens.into()),
            Some(info.bounce),
        ),
        MsgInfo::ExtIn(info) => (None, Some(convert_addr(&info.dst)), None, None),
        MsgInfo::ExtOut(info) => (Some(convert_addr(&info.src)), None, None, None),
    };

    Ok(MsgMeta {
        msg_hash: hash,
        msg_boc_hash: hash,
        src,
        dst,
        value,
        bounce,
        created_lt: None,
        created_at: None,
    })
}

const fn convert_addr(addr: &tycho_types::models::IntAddr) -> Addr {
    let mut bytes = [0u8; 32];
    let (workchain, address) = match addr {
        tycho_types::models::IntAddr::Std(std) => (std.workchain as i32, std.address.0),
        tycho_types::models::IntAddr::Var(var) => (var.workchain, {
            // skipped from TVM 11
            [0u8; 32]
        }),
    };
    bytes.copy_from_slice(&address);
    Addr {
        workchain,
        addr: bytes,
    }
}

fn create_dev_block_boc(seqno: Seqno, tx_hash: Hash256) -> anyhow::Result<BocBytes> {
    let mut builder = CellBuilder::new();
    builder.store_u32(seqno)?;
    builder.store_u256(&tycho_types::prelude::HashBytes(tx_hash.0))?;
    let cell = builder.build()?;
    Ok(Boc::encode(cell))
}
