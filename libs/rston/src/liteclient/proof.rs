//! Offline verification of LiteAPI responses against explicit block identities.

use super::{
    AccountInfo, BlockHeader, BlockLookup, GetMethodResult, TransactionCursor, TransactionPage,
    TransactionRecord, VerifiedBlock,
};
use crate::boc::Boc;
use crate::cell::{Cell, CellBuilder, CellFamily, CellSlice, HashBytes, Load, LoadCell};
use crate::dict::{AugDict, Dict};
use crate::error::Error;
use crate::merkle::MerkleProof;
use crate::models::BlockId;
use crate::models::{
    Block, BlockInfo, BlockSignature, BlockSignatureExt, BlockchainConfig, BlockchainConfigParams,
    CurrencyCollection, DepthBalanceInfo, KeyBlockRef, KeyMaxLt, OptionalAccount, ShardFees,
    ShardHashes, ShardIdent, Signature, SimplexBlockSignatures, StdAddr, Transaction,
    ValidatorBaseInfo, ValidatorInfo,
};
use crate::tl::common::{BlockIdExt, BlockLink, SignatureSet};
use crate::tl::response;

const MAX_PROOF_BYTES: usize = 8 * 1024 * 1024;

/// Malformed proof data or a mismatch with the caller's authenticated object.
///
/// Missing and pruned required paths are errors, never evidence of absence.
/// The source error is retained for BoC and cell-decoding failures.
#[derive(Debug, thiserror::Error)]
pub enum ProofError {
    /// A serialized proof or payload is not a valid BoC.
    #[error("invalid proof BoC: {0}")]
    Boc(#[from] crate::boc::de::Error),
    /// Required cells cannot be read, or signature validation failed.
    #[error("invalid proof cell: {0}")]
    Cell(#[from] Error),
    /// An authenticated path does not establish the requested protocol claim.
    #[error("{0}")]
    Invalid(&'static str),
    /// A remote execution failed; its reported exit code is not authenticated.
    #[error("remote get method exited with code {0}")]
    ContractExecution(i32),
}

pub(super) fn wire_id(id: &BlockId) -> BlockIdExt {
    id.into()
}

pub(super) fn block_id(id: &BlockIdExt) -> Result<BlockId, ProofError> {
    id.try_into()
        .map_err(|_| ProofError::Invalid("invalid shard identifier"))
}

pub(super) fn require_masterchain(id: &BlockId) -> Result<(), ProofError> {
    if id.shard != ShardIdent::MASTERCHAIN || id.seqno == 0 {
        return Err(ProofError::Invalid(
            "a nonzero masterchain block is required",
        ));
    }
    Ok(())
}

impl response::BlockData {
    /// Authenticate downloaded bytes against the caller's trusted identity.
    /// Both the serialized file hash and cell root hash must match. BlockInfo
    /// coordinates are checked; other model structures remain lazily decoded.
    pub fn verify(&self, block: &VerifiedBlock) -> Result<Block, ProofError> {
        if block_id(&self.id)? != *block.id() {
            return Err(ProofError::Invalid(
                "block response has a different block id",
            ));
        }
        let root = decode_cell(&self.data)?;
        if Boc::file_hash(&self.data) != block.id().file_hash {
            return Err(ProofError::Invalid("block file hash differs from its id"));
        }
        if root.repr_hash() != &block.id().root_hash {
            return Err(ProofError::Invalid("block root hash differs from its id"));
        }
        let decoded: Block = root.parse()?;
        let info = decoded.load_info()?;
        if info.shard != block.id().shard || info.seqno != block.id().seqno {
            return Err(ProofError::Invalid(
                "block header coordinates differ from its id",
            ));
        }
        Ok(decoded)
    }
}

impl response::BlockHeader {
    /// Authenticate a complete header against the caller's trusted identity.
    /// Other block subtrees may be pruned. The proof commits only the root hash;
    /// matching the response's file hash does not validate serialized block bytes.
    pub fn verify(&self, block: &VerifiedBlock) -> Result<BlockInfo, ProofError> {
        if block_id(&self.id)? != *block.id() {
            return Err(ProofError::Invalid(
                "header response has a different block id",
            ));
        }
        let view = checked_block(decode_cell(&self.header_proof)?, block.id())?;
        require_complete(&view.info)?;
        Ok(view.load_info()?)
    }

    /// Check lookup consistency without authenticating membership in a chain.
    /// The returned root hash is a server claim. Time checks establish only the
    /// upper lookup bound: this response lacks the predecessor needed to prove
    /// that the returned block is the first one satisfying the criterion.
    pub fn verify_lookup(
        &self,
        shard: ShardIdent,
        lookup: BlockLookup,
    ) -> Result<BlockHeader, ProofError> {
        let id = block_id(&self.id)?;
        if !id.shard.is_ancestor_of(&shard) {
            return Err(ProofError::Invalid(
                "lookup response is outside the requested shard",
            ));
        }
        let view = checked_block(decode_cell(&self.header_proof)?, &id)?;
        require_complete(&view.info)?;
        let info = view.load_info()?;
        let matches = match lookup {
            BlockLookup::Seqno(seqno) => id.seqno == seqno,
            BlockLookup::LogicalTime(lt) => info.end_lt > lt,
            BlockLookup::UnixTime(utime) => info.gen_utime >= utime,
        };
        if !matches {
            return Err(ProofError::Invalid(
                "lookup header does not satisfy the requested criterion",
            ));
        }
        Ok(BlockHeader { id, info })
    }
}

impl response::AccountState {
    /// Authenticate account state or absence without making another network query.
    /// The caller's block must already be trusted or verified. Hashes, address,
    /// shard ancestry, and all currencies in the balance are checked.
    pub fn verify(
        &self,
        block: &VerifiedBlock,
        address: &StdAddr,
    ) -> Result<AccountInfo, ProofError> {
        if block_id(&self.id)? != *block.id() {
            return Err(ProofError::Invalid(
                "account response has a different block id",
            ));
        }
        let shard_block = block_id(&self.shardblk)?;
        let accounts = account_proof(
            block.id(),
            address,
            &shard_block,
            &self.shard_proof,
            &self.proof,
        )?;
        let entry = accounts.get(address.address)?;
        let Some((balance, shard_account)) = entry else {
            if !self.state.is_empty() {
                return Err(ProofError::Invalid(
                    "account state contradicts proven absence",
                ));
            }
            return Ok(AccountInfo {
                block: *block.id(),
                shard_block: VerifiedBlock::verified(shard_block),
                account: None,
                last_transaction_lt: 0,
                last_transaction_hash: HashBytes::ZERO,
            });
        };

        let state = decode_cell(&self.state)?;
        require_complete(&state)?;
        if state.repr_hash() != shard_account.account.repr_hash() {
            return Err(ProofError::Invalid(
                "account state hash does not match its proof",
            ));
        }
        let OptionalAccount(account) = state.parse()?;
        let account = account.ok_or(ProofError::Invalid("present account has an empty state"))?;
        if account.address != crate::models::IntAddr::Std(address.clone())
            || account.balance != balance.balance
        {
            return Err(ProofError::Invalid(
                "account address or balance does not match its proof",
            ));
        }
        Ok(AccountInfo {
            block: *block.id(),
            shard_block: VerifiedBlock::verified(shard_block),
            account: Some(account),
            last_transaction_lt: shard_account.last_trans_lt,
            last_transaction_hash: shard_account.last_trans_hash,
        })
    }
}

impl response::ConfigInfo {
    /// Authenticate config parameters through the block's committed state root.
    /// An empty selection returns the config dictionary from the proof; otherwise
    /// only the requested parameters are returned and each must be fully present.
    pub fn verify(
        &self,
        block: &VerifiedBlock,
        parameters: &[i32],
    ) -> Result<BlockchainConfig, ProofError> {
        require_masterchain(block.id())?;
        if block_id(&self.id)? != *block.id() {
            return Err(ProofError::Invalid(
                "config response has a different block id",
            ));
        }
        let state = block_state(
            block.id(),
            decode_cell(&self.state_proof)?,
            decode_cell(&self.config_proof)?,
        )?;
        let extra = state.masterchain_extra()?;
        if parameters.is_empty() {
            // A full-config operation must not silently return a pruned subset.
            for entry in extra.config.params.as_dict().iter() {
                let (_, value) = entry?;
                require_complete(&value)?;
            }
            return Ok(extra.config);
        }

        let mut selected = Dict::<u32, Cell>::new();
        for &parameter in parameters {
            let key = parameter as u32;
            let value = extra
                .config
                .get_raw_cell(key)?
                .ok_or(ProofError::Invalid("requested config parameter is absent"))?;
            require_complete(&value)?;
            selected.set(key, value)?;
        }
        let root = selected
            .into_root()
            .ok_or(ProofError::Invalid("empty config selection"))?;
        Ok(BlockchainConfig {
            address: extra.config.address,
            params: BlockchainConfigParams::from_raw(root),
        })
    }
}

impl response::TransactionInfo {
    /// Authenticate one transaction at the requested address and logical time.
    /// The block must be the containing shard block; proof inclusion alone does
    /// not substitute for matching the original address and logical time.
    pub fn verify(
        &self,
        block: &VerifiedBlock,
        address: &StdAddr,
        lt: u64,
    ) -> Result<Transaction, ProofError> {
        if address.anycast.is_some()
            || !block.id().shard.contains_address(address)
            || block_id(&self.id)? != *block.id()
        {
            return Err(ProofError::Invalid(
                "transaction response does not match the requested block or address",
            ));
        }
        let cell = decode_cell(&self.transaction)?;
        // Message bodies can contain application Merkle proofs whose pruned
        // branches are part of the original committed transaction payload.
        require_complete_payload(&cell, true)?;
        let transaction: Transaction = cell.parse()?;
        if transaction.account != address.address || transaction.lt != lt {
            return Err(ProofError::Invalid(
                "transaction address or logical time differs from the request",
            ));
        }
        let proof = checked_block(decode_cell(&self.proof)?, block.id())?;
        let accounts = proof
            .extra
            .parse::<BlockExtraProof>()?
            .account_blocks
            .parse::<AugDict<HashBytes, CurrencyCollection, AccountBlockProof>>()?;
        let (_, account) = accounts.get(address.address)?.ok_or(ProofError::Invalid(
            "account is missing from transaction proof",
        ))?;
        if account.account != address.address {
            return Err(ProofError::Invalid("account block has a different address"));
        }
        let (_, included) = account.transactions.get(lt)?.ok_or(ProofError::Invalid(
            "transaction is missing from account block",
        ))?;
        if included.repr_hash() != cell.repr_hash() {
            return Err(ProofError::Invalid(
                "transaction hash differs from inclusion proof",
            ));
        }
        Ok(transaction)
    }
}

pub(super) fn check_transaction_count(count: u32) -> Result<(), ProofError> {
    if !(1..=256).contains(&count) {
        return Err(ProofError::Invalid("history page count must be 1..=256"));
    }
    Ok(())
}

impl response::TransactionList {
    /// Authenticate a page against an independently trusted account cursor.
    /// The BoC must contain 1..=count complete transactions in newest-first wire
    /// order. Hashes, account ids, logical times, and predecessor links are checked.
    /// Returned records are oldest first, with a cursor for further pagination.
    /// Server-reported block identifiers do not acquire trust from this check.
    pub fn verify(
        &self,
        cursor: &TransactionCursor,
        count: u32,
    ) -> Result<TransactionPage, ProofError> {
        check_transaction_count(count)?;
        if self.transactions.is_empty() {
            return Err(ProofError::Invalid(
                "empty history response for a nonempty cursor",
            ));
        }
        if self.transactions.len() > MAX_PROOF_BYTES {
            return Err(ProofError::Invalid("history payload exceeds 8 MiB"));
        }
        let header = crate::boc::de::BocHeader::decode(
            &self.transactions,
            &crate::boc::de::Options {
                min_roots: Some(1),
                max_roots: Some(count as usize),
            },
        )?;
        let cells = header.finalize(Cell::empty_context())?;
        let mut next = Some(cursor.clone());
        let mut transactions = Vec::with_capacity(header.roots().len());
        for &root in header.roots() {
            let expected = next
                .take()
                .ok_or(ProofError::Invalid("history continues past chain end"))?;
            let cell = cells
                .get(root)
                .ok_or(ProofError::Invalid("history root cell is missing"))?;
            // Message bodies can contain application Merkle proofs whose pruned
            // branches are part of the original committed transaction payload.
            require_complete_payload(&cell, true)?;
            if cell.repr_hash() != expected.hash() {
                return Err(ProofError::Invalid(
                    "transaction hash does not match history cursor",
                ));
            }
            let transaction: Transaction = cell.parse()?;
            if transaction.account != cursor.address().address || transaction.lt != expected.lt() {
                return Err(ProofError::Invalid(
                    "transaction account or logical time differs from history cursor",
                ));
            }
            if transaction.prev_trans_lt >= transaction.lt
                || (transaction.prev_trans_lt == 0)
                    != (transaction.prev_trans_hash == HashBytes::ZERO)
            {
                return Err(ProofError::Invalid(
                    "invalid transaction predecessor cursor",
                ));
            }
            if transaction.prev_trans_lt != 0 {
                next = Some(TransactionCursor::from_trusted(
                    cursor.address().clone(),
                    transaction.prev_trans_lt,
                    transaction.prev_trans_hash,
                )?);
            }
            transactions.push(TransactionRecord { cell, transaction });
        }
        transactions.reverse();
        Ok(TransactionPage { transactions, next })
    }
}

impl response::RunMethodResult {
    /// Authenticate the source account state, then decode the server's result.
    /// This does not verify TVM execution, output values, or the reported exit code.
    pub fn verify(
        &self,
        block: &VerifiedBlock,
        address: &StdAddr,
    ) -> Result<GetMethodResult, ProofError> {
        if block_id(&self.id)? != *block.id() {
            return Err(ProofError::Invalid(
                "get-method response has a different block id",
            ));
        }
        if self.exit_code != 0 && self.exit_code != 1 {
            return Err(ProofError::ContractExecution(self.exit_code));
        }
        let shard_block = block_id(&self.shardblk)?;
        let accounts = account_proof(
            block.id(),
            address,
            &shard_block,
            self.shard_proof.as_deref().unwrap_or_default(),
            self.proof
                .as_deref()
                .ok_or(ProofError::Invalid("missing account proof"))?,
        )?;
        let (_, account) = accounts
            .get(address.address)?
            .ok_or(ProofError::Invalid("get-method account is absent"))?;
        let account_state_hash = *account.account.repr_hash();
        let state_proof = self
            .state_proof
            .as_deref()
            .ok_or(ProofError::Invalid("missing get-method state proof"))?;
        unwrap_proof(decode_cell(state_proof)?, &account_state_hash)?;
        let stack = match self.result.as_deref() {
            Some(bytes) if !bytes.is_empty() => decode_cell(bytes)?.parse()?,
            _ => crate::models::vm::Tuple::empty(),
        };
        Ok(GetMethodResult {
            block: *block.id(),
            shard_block: VerifiedBlock::verified(shard_block),
            account_state_hash,
            exit_code: self.exit_code,
            stack,
        })
    }
}

impl response::PartialBlockProof {
    /// Authenticate every link in a partial chain from an explicit checkpoint.
    ///
    /// The result can be an intermediate block. Partial responses must progress
    /// toward the requested target; complete responses must reach its full id.
    /// Forward paths can first go backwards to a committed key block. At most
    /// 128 links are accepted per response. No network calls or trust updates occur.
    pub fn verify(
        &self,
        known: &VerifiedBlock,
        target: &BlockId,
    ) -> Result<VerifiedBlock, ProofError> {
        require_masterchain(known.id())?;
        require_masterchain(target)?;
        if block_id(&self.from)? != *known.id() || self.steps.len() > 128 {
            return Err(ProofError::Invalid(
                "incorrect proof start or too many links",
            ));
        }
        let forward = target.seqno > known.id().seqno;
        let mut current = *known.id();
        for step in &self.steps {
            match step {
                BlockLink::BlockLinkBack {
                    from,
                    to,
                    to_key_block,
                    dest_proof,
                    proof,
                    state_proof,
                } => {
                    if block_id(from)? != current {
                        return Err(ProofError::Invalid("backward proof link is disconnected"));
                    }
                    let next = block_id(to)?;
                    verify_backward(
                        &current,
                        &next,
                        *to_key_block,
                        dest_proof,
                        proof,
                        state_proof,
                    )?;
                    current = next;
                }
                BlockLink::BlockLinkForward {
                    from,
                    to,
                    to_key_block,
                    dest_proof,
                    config_proof,
                    signatures,
                } => {
                    if !forward || block_id(from)? != current {
                        return Err(ProofError::Invalid(
                            "forward proof link is disconnected or has the wrong direction",
                        ));
                    }
                    let next = block_id(to)?;
                    verify_forward(
                        &current,
                        &next,
                        *to_key_block,
                        dest_proof,
                        config_proof,
                        signatures,
                    )?;
                    current = next;
                }
            }
        }
        if current != block_id(&self.to)? || self.complete != (current == *target) {
            return Err(ProofError::Invalid(
                "proof destination or completion flag is incorrect",
            ));
        }
        if current == *target {
            return Ok(VerifiedBlock::verified(current));
        }
        let progressed = if forward {
            current.seqno > known.id().seqno && current.seqno <= target.seqno
        } else {
            current.seqno < known.id().seqno && current.seqno >= target.seqno
        };
        if !progressed {
            return Err(ProofError::Invalid(
                "partial proof did not progress toward the target",
            ));
        }
        Ok(VerifiedBlock::verified(current))
    }
}

fn decode_cell(bytes: &[u8]) -> Result<Cell, ProofError> {
    if bytes.len() > MAX_PROOF_BYTES {
        return Err(ProofError::Invalid("proof payload exceeds 8 MiB"));
    }
    Ok(Boc::decode(bytes)?)
}

fn decode_pair(bytes: &[u8]) -> Result<(Cell, Cell), ProofError> {
    if bytes.len() > MAX_PROOF_BYTES {
        return Err(ProofError::Invalid("proof payload exceeds 8 MiB"));
    }
    Ok(Boc::decode_pair(bytes)?)
}

fn unwrap_proof(cell: Cell, expected: &HashBytes) -> Result<Cell, ProofError> {
    let proof = MerkleProof::load_from_cell(cell.as_ref())?;
    if proof.hash != *expected {
        return Err(ProofError::Invalid(
            "Merkle proof root hash does not match the expected object",
        ));
    }
    Ok(Cell::virtualize(proof.cell))
}

fn checked_block(cell: Cell, id: &BlockId) -> Result<BlockProofView, ProofError> {
    let block: BlockProofView = unwrap_proof(cell, &id.root_hash)?.parse()?;
    if block.info.cell_type() != crate::cell::CellType::PrunedBranch {
        let info = block.load_info()?;
        if info.shard != id.shard || info.seqno != id.seqno {
            return Err(ProofError::Invalid(
                "block header coordinates differ from its id",
            ));
        }
    }
    Ok(block)
}

fn block_state(
    id: &BlockId,
    block_proof: Cell,
    state_proof: Cell,
) -> Result<StateProof, ProofError> {
    let block = checked_block(block_proof, id)?;
    let state_hash =
        crate::merkle::MerkleUpdate::load_from_cell(block.state_update.as_ref())?.new_hash;
    let state = StateProof::load_from(&mut unwrap_proof(state_proof, &state_hash)?.as_slice()?)?;
    if state.shard != id.shard || state.seqno != id.seqno {
        return Err(ProofError::Invalid(
            "state coordinates differ from its block",
        ));
    }
    Ok(state)
}

fn account_proof(
    block: &BlockId,
    address: &StdAddr,
    shard: &BlockId,
    shard_proof: &[u8],
    proof: &[u8],
) -> Result<AugDict<HashBytes, DepthBalanceInfo, ShardAccountProof>, ProofError> {
    if address.anycast.is_some() || !shard.shard.contains_address(address) {
        return Err(ProofError::Invalid(
            "account address is outside the proven shard",
        ));
    }
    if block.shard == ShardIdent::MASTERCHAIN && i32::from(address.workchain) != -1 {
        let (master, state) = decode_pair(shard_proof)?;
        let extra = block_state(block, master, state)?.masterchain_extra()?;
        if extra.shard_for(address)? != *shard {
            return Err(ProofError::Invalid(
                "shard descriptor does not match the account's shard block",
            ));
        }
    } else if *block != *shard {
        return Err(ProofError::Invalid(
            "account response has a different shard block",
        ));
    }
    let (header, state) = decode_pair(proof)?;
    let state = block_state(shard, header, state)?;
    Ok(state.accounts.parse()?)
}

// Proof views read only committed paths needed for authentication. Unrelated
// references may be pruned; the hashes are checked before these views are read.
#[derive(Load)]
#[tlb(tag = "#11ef55aa")]
struct BlockProofView {
    _global_id: i32,
    info: Cell,
    _value_flow: Cell,
    state_update: Cell,
    extra: Cell,
}

impl BlockProofView {
    fn load_info(&self) -> Result<BlockInfo, Error> {
        self.info.parse()
    }
}

#[derive(Load)]
#[tlb(tag = "#4a33f6fd")]
struct BlockExtraProof {
    _in_messages: Cell,
    _out_messages: Cell,
    account_blocks: Cell,
    _rand_seed: HashBytes,
    _created_by: HashBytes,
    custom: Option<Cell>,
}

#[derive(Load)]
struct ShardAccountProof {
    account: Cell,
    last_trans_hash: HashBytes,
    last_trans_lt: u64,
}

struct AccountBlockProof {
    account: HashBytes,
    transactions: AugDict<u64, CurrencyCollection, Cell>,
}

impl<'a> Load<'a> for AccountBlockProof {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if slice.load_small_uint(4)? != 5 {
            return Err(Error::InvalidTag);
        }
        let account = slice.load_u256()?;
        let transactions = AugDict::load_from_root_ext(slice, Cell::empty_context())?;
        slice.load_reference()?; // account state update
        Ok(Self {
            account,
            transactions,
        })
    }
}

struct StateProof {
    shard: ShardIdent,
    seqno: u32,
    accounts: Cell,
    custom: Option<Cell>,
}

impl<'a> Load<'a> for StateProof {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if slice.load_u32()? != 0x9023afe2 {
            return Err(Error::InvalidTag);
        }
        slice.load_u32()?; // global_id
        let shard = ShardIdent::load_from(slice)?;
        let seqno = slice.load_u32()?;
        slice.skip_first(32 + 32 + 64 + 32, 0)?;
        slice.load_reference()?; // out_msg_queue_info
        slice.load_bit()?; // before_split
        let accounts = slice.load_reference_cloned()?;
        slice.load_reference()?; // balances, libraries, and master_ref
        let custom = Option::<Cell>::load_from(slice)?;
        Ok(Self {
            shard,
            seqno,
            accounts,
            custom,
        })
    }
}

impl StateProof {
    fn masterchain_extra(&self) -> Result<MasterchainExtraProof, ProofError> {
        if self.shard != ShardIdent::MASTERCHAIN {
            return Err(ProofError::Invalid("masterchain state is required"));
        }
        let custom = self
            .custom
            .as_ref()
            .ok_or(ProofError::Invalid("masterchain state has no extra data"))?;
        Ok(MasterchainExtraProof::load_from(&mut custom.as_slice()?)?)
    }
}

struct MasterchainExtraProof {
    shards: ShardHashes,
    config: BlockchainConfig,
    info: Cell,
}

impl<'a> Load<'a> for MasterchainExtraProof {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if slice.load_u16()? != 0xcc26 {
            return Err(Error::InvalidTag);
        }
        Ok(Self {
            shards: ShardHashes::load_from(slice)?,
            config: BlockchainConfig::load_from(slice)?,
            info: slice.load_reference_cloned()?,
        })
    }
}

impl MasterchainExtraProof {
    fn shard_for(&self, address: &StdAddr) -> Result<BlockId, ProofError> {
        let workchain = i32::from(address.workchain);
        let tree = self
            .shards
            .get_workchain_shards(workchain)?
            .ok_or(ProofError::Invalid(
                "account workchain is absent from shard proof",
            ))?;
        let mut shard = ShardIdent::new(workchain, ShardIdent::PREFIX_FULL)
            .ok_or(ProofError::Invalid("invalid account workchain"))?;
        let mut slice = tree.root().as_slice()?;
        while slice.load_bit()? {
            let (left, right) = shard
                .split()
                .ok_or(ProofError::Invalid("shard proof tree is too deep"))?;
            let index = if left.contains_address(address) {
                shard = left;
                0
            } else {
                shard = right;
                1
            };
            slice = slice.get_reference(index)?.as_slice()?;
        }
        match slice.load_small_uint(4)? {
            0xa | 0xb => {}
            _ => return Err(Error::InvalidTag.into()),
        }
        let seqno = slice.load_u32()?;
        slice.skip_first(32 + 64 + 64, 0)?;
        Ok(BlockId {
            shard,
            seqno,
            root_hash: slice.load_u256()?,
            file_hash: slice.load_u256()?,
        })
    }
}

fn verify_backward(
    from: &BlockId,
    to: &BlockId,
    to_key: bool,
    dest_proof: &[u8],
    proof: &[u8],
    state_proof: &[u8],
) -> Result<(), ProofError> {
    require_masterchain(from)?;
    require_masterchain(to)?;
    if to.seqno >= from.seqno {
        return Err(ProofError::Invalid("backward link does not decrease seqno"));
    }
    let dest = checked_block(decode_cell(dest_proof)?, to)?;
    if dest.load_info()?.key_block != to_key {
        return Err(ProofError::Invalid(
            "destination key-block flag is incorrect",
        ));
    }
    let extra =
        block_state(from, decode_cell(proof)?, decode_cell(state_proof)?)?.masterchain_extra()?;
    let mut info = extra.info.as_slice()?;
    if info.load_u16()? > 3 {
        return Err(ProofError::Invalid("unsupported masterchain extra flags"));
    }
    ValidatorInfo::load_from(&mut info)?;
    let previous = AugDict::<u32, KeyMaxLt, KeyBlockRef>::load_from(&mut info)?;
    let (_, entry) = previous.get(to.seqno)?.ok_or(ProofError::Invalid(
        "destination is absent from committed block history",
    ))?;
    if entry.is_key_block != to_key
        || entry.block_ref.seqno != to.seqno
        || entry.block_ref.root_hash != to.root_hash
        || entry.block_ref.file_hash != to.file_hash
    {
        return Err(ProofError::Invalid(
            "destination differs from committed block history",
        ));
    }
    Ok(())
}

fn verify_forward(
    from: &BlockId,
    to: &BlockId,
    to_key: bool,
    dest_proof: &[u8],
    config_proof: &[u8],
    signatures: &SignatureSet,
) -> Result<(), ProofError> {
    require_masterchain(from)?;
    require_masterchain(to)?;
    if to.seqno <= from.seqno {
        return Err(ProofError::Invalid("forward link does not increase seqno"));
    }
    let dest = checked_block(decode_cell(dest_proof)?, to)?;
    let info = dest.load_info()?;
    if info.key_block != to_key {
        return Err(ProofError::Invalid(
            "destination key-block flag is incorrect",
        ));
    }
    let source = checked_block(decode_cell(config_proof)?, from)?;
    if source.info.cell_type() != crate::cell::CellType::PrunedBranch
        && !source.load_info()?.key_block
    {
        return Err(ProofError::Invalid(
            "forward link must authenticate validator config from a key block",
        ));
    }
    let extra = source
        .extra
        .parse::<BlockExtraProof>()?
        .custom
        .ok_or(ProofError::Invalid("key block has no masterchain extra"))?;
    let mut slice = extra.as_slice()?;
    if slice.load_u16()? != 0xcca5 || !slice.load_bit()? {
        return Err(ProofError::Invalid("key block has no configuration"));
    }
    ShardHashes::load_from(&mut slice)?;
    ShardFees::load_from(&mut slice)?;
    slice.load_reference()?; // previous signatures and special messages
    let config = BlockchainConfig::load_from(&mut slice)?;
    verify_signatures(to, &info, &config, signatures)
}

fn verify_signatures(
    block: &BlockId,
    info: &BlockInfo,
    config: &BlockchainConfig,
    signature_set: &SignatureSet,
) -> Result<(), ProofError> {
    let catchain = config.get_catchain_config()?;
    let validator_set = config.get_current_validator_set()?;
    let (validators, subset_hash) = validator_set
        .compute_subset(block.shard, &catchain, info.gen_catchain_seqno)
        .ok_or(ProofError::Invalid("invalid masterchain validator subset"))?;
    let (seqno, hash, signatures) = match signature_set {
        SignatureSet::Ordinary {
            catchain_seqno,
            validator_set_hash,
            signatures,
        } => (
            *catchain_seqno as u32,
            *validator_set_hash as u32,
            signatures,
        ),
        SignatureSet::Simplex {
            cc_seqno,
            validator_set_hash,
            signatures,
            ..
        } => (*cc_seqno as u32, *validator_set_hash as u32, signatures),
    };
    if signatures.is_empty()
        || signatures.len() > validators.len()
        || seqno != info.gen_catchain_seqno
        || hash != info.gen_validator_list_hash_short
        || hash != subset_hash
    {
        return Err(ProofError::Invalid(
            "signature set does not match validator metadata",
        ));
    }

    let mut nodes = std::collections::HashMap::new();
    let mut total_weight = 0u64;
    for validator in &validators {
        let node = tl_proto::hash(tycho_crypto::tl::PublicKey::Ed25519 {
            key: validator.public_key.as_ref(),
        });
        if validator.weight == 0 || nodes.insert(node, validator.weight).is_some() {
            return Err(ProofError::Invalid(
                "validator subset has duplicate keys or zero weight",
            ));
        }
        total_weight = total_weight
            .checked_add(validator.weight)
            .ok_or(ProofError::Invalid("validator weights overflow"))?;
    }

    let mut dict = Dict::new();
    let mut signed_weight = 0u64;
    for (index, signature) in signatures.iter().enumerate() {
        let weight = nodes
            .remove(&signature.node_id_short.0)
            .ok_or(ProofError::Invalid("duplicate or unknown signature signer"))?;
        signed_weight = signed_weight
            .checked_add(weight)
            .ok_or(ProofError::Invalid("signature weights overflow"))?;
        let bytes = signature
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| ProofError::Invalid("Ed25519 signature must contain 64 bytes"))?;
        let index = u16::try_from(index).map_err(|_| ProofError::Invalid("too many signatures"))?;
        dict.set(
            index,
            BlockSignature {
                node_id_short: HashBytes(signature.node_id_short.0),
                signature: Signature(bytes),
            },
        )?;
    }
    if u128::from(signed_weight) * 3 <= u128::from(total_weight) * 2 {
        return Err(ProofError::Invalid("insufficient signature weight"));
    }

    match signature_set {
        SignatureSet::Ordinary { .. } => {
            dict.check_signatures(&validators, &Block::build_data_for_sign(block))?;
        }
        SignatureSet::Simplex {
            session_id,
            slot,
            candidate,
            ..
        } => {
            if candidate.len() > 160 {
                return Err(ProofError::Invalid("Simplex candidate exceeds 160 bytes"));
            }
            let mut next = Cell::empty_cell();
            for chunk in candidate.chunks(127).rev() {
                let mut builder = CellBuilder::new();
                builder.store_raw(chunk, (chunk.len() * 8) as u16)?;
                if next.bit_len() != 0 {
                    builder.store_reference(next)?;
                }
                next = builder.build()?;
            }
            SimplexBlockSignatures {
                validator_info: ValidatorBaseInfo {
                    catchain_seqno: seqno,
                    validator_list_hash_short: hash,
                },
                signature_count: signatures.len() as u32,
                total_weight: signed_weight,
                signatures: dict,
                session_id: HashBytes(session_id.0),
                slot: *slot as u32,
                candidate_data: next,
            }
            .check_signatures(
                block,
                &ValidatorBaseInfo {
                    catchain_seqno: info.gen_catchain_seqno,
                    validator_list_hash_short: info.gen_validator_list_hash_short,
                },
                &validators,
            )?;
        }
    }
    Ok(())
}

fn require_complete(cell: &Cell) -> Result<(), ProofError> {
    require_complete_payload(cell, false)
}

fn require_complete_payload(cell: &Cell, allow_merkle_payloads: bool) -> Result<(), ProofError> {
    // A pruned value authenticates a hash, but cannot supply a complete payload.
    let mut visited = std::collections::HashSet::new();
    let mut pending = vec![cell.as_ref()];
    while let Some(cell) = pending.pop() {
        if cell.cell_type() == crate::cell::CellType::PrunedBranch {
            return Err(ProofError::Invalid(
                "requested payload contains pruned data",
            ));
        }
        if !visited.insert(*cell.repr_hash()) {
            continue;
        }
        if allow_merkle_payloads
            && matches!(
                cell.cell_type(),
                crate::cell::CellType::MerkleProof | crate::cell::CellType::MerkleUpdate
            )
        {
            continue;
        }
        for child in cell.references() {
            pending.push(child);
        }
    }
    Ok(())
}
