#![cfg(feature = "liteclient")]

use super::TestServer;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, CellFamily, CellType, DynCell, HashBytes};
use rston::liteclient::{FromResponse, LiteError, MethodId, ProofError, VerifiedBlock};
use rston::merkle::MerkleProof;
use rston::models::vm::{Tuple, TupleItem};
use rston::models::{
    BlockId, ExtInMsgInfo, IntAddr, IntMsgInfo, Message, MsgInfo, StdAddr, Transaction,
};
use rston::num::Tokens;
use rston::tl::common::{BlockLink, Int256, SignatureSet};
use rston::tl::{Request, Response, response};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const ACCOUNT: &[u8] = include_bytes!("../fixtures/liteclient/account.tl");
const SHARD_ACCOUNT: &[u8] = include_bytes!("../fixtures/liteclient/shard-account.tl");
const ABSENT_ACCOUNT: &[u8] = include_bytes!("../fixtures/liteclient/absent-account.tl");
const CONFIG: &[u8] = include_bytes!("../fixtures/liteclient/config.tl");
const CONFIG_ALL: &[u8] = include_bytes!("../fixtures/liteclient/config-all.tl");
const TRANSACTION: &[u8] = include_bytes!("../fixtures/liteclient/transaction.tl");
const RUN_METHOD: &[u8] = include_bytes!("../fixtures/liteclient/run-method.tl");
const FORWARD: &[u8] = include_bytes!("../fixtures/liteclient/forward.tl");
const BACKWARD: &[u8] = include_bytes!("../fixtures/liteclient/backward.tl");
const ORDINARY: &[u8] = include_bytes!("../fixtures/liteclient/ordinary.tl");
const ORDINARY_BOUNDARY: &[u8] = include_bytes!("../fixtures/liteclient/ordinary-boundary.tl");

#[derive(Deserialize)]
struct Fixture {
    #[serde(deserialize_with = "from_string")]
    head: BlockId,
    #[serde(deserialize_with = "from_string")]
    checkpoint: BlockId,
    #[serde(deserialize_with = "from_string")]
    transaction_block: BlockId,
    transaction_lt: u64,
    #[serde(deserialize_with = "from_string")]
    elector: StdAddr,
    #[serde(deserialize_with = "from_string")]
    shard_account_address: StdAddr,
    #[serde(deserialize_with = "from_string")]
    absent: StdAddr,
    elector_balance: String,
    elector_last_lt: u64,
    shard_account_balance: String,
    transaction_hash: String,
    method_value: String,
    config_hashes: std::collections::BTreeMap<String, String>,
    method_ids: std::collections::BTreeMap<String, u64>,
    #[serde(deserialize_with = "from_string")]
    ordinary_from: BlockId,
    #[serde(deserialize_with = "from_string")]
    ordinary_to: BlockId,
    #[serde(deserialize_with = "from_string")]
    ordinary_boundary_from: BlockId,
    #[serde(deserialize_with = "from_string")]
    ordinary_boundary_to: BlockId,
}

fn from_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    String::deserialize(deserializer)?
        .parse()
        .map_err(serde::de::Error::custom)
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/liteclient/manifest.json")).unwrap()
}

fn reply<T: FromResponse>(bytes: &[u8]) -> T {
    T::from_response(tl_proto::deserialize(bytes).unwrap()).unwrap()
}

#[test]
fn verifies_mainnet_forward_and_backward_proof_chains() {
    let fixture = fixture();
    let forward: response::PartialBlockProof = reply(FORWARD);
    let checkpoint = VerifiedBlock::from_trusted(fixture.checkpoint);
    let head = forward.verify(&checkpoint, &fixture.head).unwrap();
    assert_eq!(head.id(), &fixture.head);

    let backward: response::PartialBlockProof = reply(BACKWARD);
    let previous = backward.verify(&head, &fixture.checkpoint).unwrap();
    assert_eq!(previous, checkpoint);
}

#[test]
fn verifies_ordinary_signatures_and_rejects_exactly_two_thirds() {
    let fixture = fixture();
    let checkpoint = VerifiedBlock::from_trusted(fixture.ordinary_from);
    let proof: response::PartialBlockProof = reply(ORDINARY);
    let verified = proof.verify(&checkpoint, &fixture.ordinary_to).unwrap();
    assert_eq!(verified.id(), &fixture.ordinary_to);

    let checkpoint = VerifiedBlock::from_trusted(fixture.ordinary_boundary_from);
    let mut proof: response::PartialBlockProof = reply(ORDINARY_BOUNDARY);
    proof
        .verify(&checkpoint, &fixture.ordinary_boundary_to)
        .unwrap();
    let BlockLink::BlockLinkForward {
        signatures: SignatureSet::Ordinary { signatures, .. },
        ..
    } = &mut proof.steps[0]
    else {
        panic!("expected ordinary signatures");
    };
    signatures.pop();
    assert!(
        proof
            .verify(&checkpoint, &fixture.ordinary_boundary_to)
            .is_err()
    );
}

#[test]
fn method_selectors_and_stack_argument_order_match_go() {
    let fixture = fixture();
    for (name, expected) in fixture.method_ids {
        assert_eq!(MethodId::from_name(&name).value(), expected);
    }

    let expected = Boc::decode(include_bytes!("../fixtures/liteclient/arguments.boc")).unwrap();
    let arguments = Tuple(vec![TupleItem::Int(11.into()), TupleItem::Int(22.into())]);
    let encoded = CellBuilder::build_from(&arguments).unwrap();
    assert_eq!(encoded.repr_hash(), expected.repr_hash());
    assert_eq!(expected.parse::<Tuple>().unwrap(), arguments);
}

#[test]
fn verifies_mainnet_accounts_in_masterchain_and_shard() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let account: response::AccountState = reply(ACCOUNT);
    let decoded = account.verify(&head, &fixture.elector).unwrap();
    assert!(decoded.account.is_some());
    assert_eq!(decoded.shard_block, head);
    assert_eq!(decoded.last_transaction_lt, fixture.elector_last_lt);
    assert_eq!(
        decoded
            .account
            .unwrap()
            .balance
            .tokens
            .into_inner()
            .to_string(),
        fixture.elector_balance
    );

    let account: response::AccountState = reply(SHARD_ACCOUNT);
    let decoded = account
        .verify(&head, &fixture.shard_account_address)
        .unwrap();
    assert!(decoded.account.is_some());
    assert_eq!(decoded.shard_block.id().shard.workchain(), 0);
    assert_eq!(
        decoded
            .account
            .unwrap()
            .balance
            .tokens
            .into_inner()
            .to_string(),
        fixture.shard_account_balance
    );
}

#[test]
fn verifies_mainnet_config_and_transaction() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let config: response::ConfigInfo = reply(CONFIG);
    let decoded = config.verify(&head, &[1, 4, 28, 34]).unwrap();
    assert_eq!(
        decoded
            .params
            .as_dict()
            .keys()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        vec![1, 4, 28, 34]
    );
    for (key, expected_hash) in fixture.config_hashes {
        let value = decoded.get_raw_cell(key.parse().unwrap()).unwrap().unwrap();
        assert_eq!(hex::encode(value.repr_hash().0), expected_hash);
    }

    let config: response::ConfigInfo = reply(CONFIG_ALL);
    config.verify(&head, &[]).unwrap();

    let transaction: response::TransactionInfo = reply(TRANSACTION);
    let block = VerifiedBlock::from_trusted(fixture.transaction_block);
    let decoded = transaction
        .verify(&block, &fixture.elector, fixture.transaction_lt)
        .unwrap();
    assert_eq!(decoded.lt, fixture.transaction_lt);
    let cell = Boc::decode(&transaction.transaction).unwrap();
    assert_eq!(hex::encode(cell.repr_hash().0), fixture.transaction_hash);
}

#[test]
fn authenticates_get_method_source_state() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let method: response::RunMethodResult = reply(RUN_METHOD);
    let decoded = method.verify(&head, &fixture.elector).unwrap();
    assert_eq!(decoded.exit_code, 0);
    assert_eq!(
        decoded.stack,
        Tuple(vec![TupleItem::Int(fixture.method_value.parse().unwrap())])
    );
}

#[test]
fn proves_absence_and_rejects_missing_proof_data() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let mut account: response::AccountState = reply(ABSENT_ACCOUNT);

    let decoded = account.verify(&head, &fixture.absent).unwrap();
    assert!(decoded.account.is_none());
    assert_eq!(decoded.last_transaction_lt, 0);
    assert_eq!(decoded.last_transaction_hash, HashBytes::ZERO);

    account.proof.clear();
    assert!(account.verify(&head, &fixture.absent).is_err());
}

#[test]
fn rejects_account_proofs_for_a_different_block_address_or_state() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let original: response::AccountState = reply(ACCOUNT);

    let mut changed = original.clone();
    changed.id.file_hash.0[0] ^= 1;
    assert!(changed.verify(&head, &fixture.elector).is_err());

    let mut address = fixture.elector.clone();
    address.address.0[0] ^= 1;
    assert!(original.verify(&head, &address).is_err());

    let mut changed = original;
    changed.state = Boc::encode(CellBuilder::build_from(0_u8).unwrap());
    assert!(changed.verify(&head, &fixture.elector).is_err());
}

#[test]
fn rejects_wrong_shard_identity_and_missing_shard_proof() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let original: response::AccountState = reply(SHARD_ACCOUNT);

    let mut changed = original.clone();
    changed.shardblk.file_hash.0[0] ^= 1;
    assert!(
        changed
            .verify(&head, &fixture.shard_account_address)
            .is_err()
    );

    let mut changed = original;
    changed.shard_proof.clear();
    assert!(
        changed
            .verify(&head, &fixture.shard_account_address)
            .is_err()
    );
}

#[test]
fn rejects_partial_or_mismatched_config_proofs() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let original: response::ConfigInfo = reply(CONFIG);

    assert!(original.verify(&head, &[]).is_err());
    assert!(original.verify(&head, &[999999]).is_err());

    let mut changed = original.clone();
    changed.id.seqno += 1;
    assert!(changed.verify(&head, &[1]).is_err());

    let mut changed = original;
    std::mem::swap(&mut changed.state_proof, &mut changed.config_proof);
    assert!(changed.verify(&head, &[1]).is_err());
}

#[test]
fn rejects_transactions_for_a_different_address_lt_or_hash() {
    let fixture = fixture();
    let block = VerifiedBlock::from_trusted(fixture.transaction_block);
    let original: response::TransactionInfo = reply(TRANSACTION);

    assert!(
        original
            .verify(&block, &fixture.elector, fixture.transaction_lt + 1)
            .is_err()
    );
    let mut address = fixture.elector.clone();
    address.address.0[0] ^= 1;
    assert!(
        original
            .verify(&block, &address, fixture.transaction_lt)
            .is_err()
    );

    let mut changed = original;
    let mut payload = Boc::decode(&changed.transaction)
        .unwrap()
        .parse::<rston::models::Transaction>()
        .unwrap();
    payload.prev_trans_hash.0[0] ^= 1;
    changed.transaction = Boc::encode(CellBuilder::build_from(payload).unwrap());
    assert!(
        changed
            .verify(&block, &fixture.elector, fixture.transaction_lt)
            .is_err()
    );
}

// Swap the real transaction leaf in its block proof and re-root the proof, so the
// replacement is included in a block that the caller trusts by its new root hash.
fn include_transaction(
    original: &response::TransactionInfo,
    block: BlockId,
    transaction: Cell,
) -> (VerifiedBlock, response::TransactionInfo) {
    fn replace(cell: &DynCell, target: &HashBytes, replacement: &Cell) -> Option<Cell> {
        if cell.hash(0) == target {
            return Some(replacement.clone());
        }
        let children = cell
            .references()
            .map(|child| replace(child, target, replacement))
            .collect::<Vec<_>>();
        if children.iter().all(Option::is_none) {
            return None;
        }
        let mut builder = CellBuilder::new();
        builder.set_exotic(cell.is_exotic());
        builder.store_cell_data(cell).unwrap();
        for (index, child) in children.into_iter().enumerate() {
            let child = child.unwrap_or_else(|| cell.reference_cloned(index as u8).unwrap());
            builder.store_reference(child).unwrap();
        }
        Some(builder.build().unwrap())
    }

    let proof = Boc::decode(&original.proof)
        .unwrap()
        .parse_exotic::<MerkleProof>()
        .unwrap();
    let target = *Boc::decode(&original.transaction).unwrap().repr_hash();
    let cell = replace(proof.cell.as_ref(), &target, &transaction).unwrap();
    let proof = MerkleProof {
        hash: *cell.hash(0),
        depth: cell.depth(0),
        cell,
    };
    let mut info = original.clone();
    info.id.root_hash = Int256(proof.hash.0);
    info.transaction = Boc::encode(transaction);
    let block = BlockId {
        root_hash: proof.hash,
        ..block
    };
    info.proof = Boc::encode(CellBuilder::build_from(proof).unwrap());
    (VerifiedBlock::from_trusted(block), info)
}

// An application Merkle proof with a pruned branch, as carried in message bodies.
fn application_proof() -> MerkleProof {
    let mut builder = CellBuilder::new();
    builder.store_reference(Cell::empty_cell()).unwrap();
    let child = builder.build().unwrap();
    let mut builder = CellBuilder::new();
    builder.store_reference(child).unwrap();
    let root = builder.build().unwrap();
    MerkleProof::create(root.as_ref(), HashSet::from([*root.repr_hash()]))
        .build()
        .unwrap()
}

// The real transaction with an external in-message whose body holds one reference.
fn transaction_with_body_reference(
    original: &response::TransactionInfo,
    address: &StdAddr,
    reference: Cell,
) -> Cell {
    let mut builder = CellBuilder::new();
    builder.store_reference(reference).unwrap();
    let body = builder.build().unwrap();
    let message = Message {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(address.clone()),
            ..Default::default()
        }),
        init: None,
        body: body.as_slice().unwrap(),
        layout: None,
    };
    let mut transaction: Transaction = Boc::decode(&original.transaction).unwrap().parse().unwrap();
    transaction.in_msg = Some(CellBuilder::build_from(message).unwrap());
    CellBuilder::build_from(&transaction).unwrap()
}

#[test]
fn accepts_application_merkle_proofs_in_a_transaction() {
    let fixture = fixture();
    let original: response::TransactionInfo = reply(TRANSACTION);
    // Pruned branches inside an application Merkle proof belong to the committed
    // message body; they are not server-side pruning of the transaction.
    let proof = CellBuilder::build_from(application_proof()).unwrap();
    let cell = transaction_with_body_reference(&original, &fixture.elector, proof);
    let (block, info) = include_transaction(&original, fixture.transaction_block, cell);

    let decoded = info
        .verify(&block, &fixture.elector, fixture.transaction_lt)
        .unwrap();
    let mut message: Message<'_> = decoded.in_msg.as_ref().unwrap().parse().unwrap();
    let embedded = message.body.load_reference().unwrap();
    assert_eq!(decoded.lt, fixture.transaction_lt);
    assert_eq!(embedded.cell_type(), CellType::MerkleProof);
    assert_eq!(
        embedded
            .reference(0)
            .and_then(|cell| cell.reference(0))
            .unwrap()
            .cell_type(),
        CellType::PrunedBranch
    );
}

#[test]
fn rejects_pruned_transaction_data_outside_merkle_proofs() {
    let fixture = fixture();
    let original: response::TransactionInfo = reply(TRANSACTION);
    // The same pruned branch without its Merkle proof cell is incomplete data.
    let cell =
        transaction_with_body_reference(&original, &fixture.elector, application_proof().cell);
    let (block, info) = include_transaction(&original, fixture.transaction_block, cell);
    assert_eq!(
        info.verify(&block, &fixture.elector, fixture.transaction_lt)
            .unwrap_err()
            .to_string(),
        "requested payload contains pruned data"
    );

    // A server-pruned transaction cannot supply its required fields either.
    let cell = Boc::decode(&original.transaction).unwrap();
    let mut partial = original;
    partial.transaction = Boc::encode(
        MerkleProof::create(cell.as_ref(), HashSet::from([*cell.repr_hash()]))
            .build()
            .unwrap()
            .cell,
    );
    let block = VerifiedBlock::from_trusted(fixture.transaction_block);
    assert_eq!(
        partial
            .verify(&block, &fixture.elector, fixture.transaction_lt)
            .unwrap_err()
            .to_string(),
        "requested payload contains pruned data"
    );
}

#[test]
fn requires_get_method_proofs_but_does_not_claim_to_verify_execution() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let original: response::RunMethodResult = reply(RUN_METHOD);

    let mut changed = original.clone();
    changed.state_proof = None;
    assert!(changed.verify(&head, &fixture.elector).is_err());

    let mut changed = original.clone();
    changed.exit_code = 11;
    assert!(matches!(
        changed.verify(&head, &fixture.elector),
        Err(ProofError::ContractExecution(11))
    ));

    let server_output = Tuple(vec![TupleItem::Int(9999.into())]);
    let mut changed = original;
    changed.exit_code = 1;
    changed.result = Some(Boc::encode(
        CellBuilder::build_from(&server_output).unwrap(),
    ));
    let decoded = changed.verify(&head, &fixture.elector).unwrap();
    assert_eq!(decoded.exit_code, 1);
    assert_eq!(decoded.stack, server_output);
}

#[test]
fn empty_remote_results_match_go() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let original: response::RunMethodResult = reply(RUN_METHOD);
    for result in [None, Some(Vec::new())] {
        let mut method = original.clone();
        method.result = result;
        assert!(
            method
                .verify(&head, &fixture.elector)
                .unwrap()
                .stack
                .is_empty()
        );
    }
}

#[test]
fn rejects_disconnected_incomplete_or_nonprogressing_proof_chains() {
    let fixture = fixture();
    let checkpoint = VerifiedBlock::from_trusted(fixture.checkpoint);
    let original: response::PartialBlockProof = reply(FORWARD);

    let mut changed = original.clone();
    changed.from.root_hash.0[0] ^= 1;
    assert!(changed.verify(&checkpoint, &fixture.head).is_err());

    let mut changed = original.clone();
    changed.complete = false;
    assert!(changed.verify(&checkpoint, &fixture.head).is_err());

    let mut changed = original.clone();
    changed.steps.clear();
    changed.to = changed.from.clone();
    changed.complete = false;
    assert!(changed.verify(&checkpoint, &fixture.head).is_err());

    let mut changed = original;
    changed.to.file_hash.0[0] ^= 1;
    assert!(changed.verify(&checkpoint, &fixture.head).is_err());
}

#[test]
fn rejects_changed_simplex_votes_and_signers() {
    let fixture = fixture();
    let checkpoint = VerifiedBlock::from_trusted(fixture.checkpoint);
    let original: response::PartialBlockProof = reply(FORWARD);

    for mutation in [
        "slot",
        "session",
        "candidate",
        "duplicate signer",
        "signature",
    ] {
        let mut changed = original.clone();
        let BlockLink::BlockLinkForward {
            signatures:
                SignatureSet::Simplex {
                    signatures,
                    session_id,
                    slot,
                    candidate,
                    ..
                },
            ..
        } = &mut changed.steps[0]
        else {
            panic!("fixture must contain a Simplex forward link");
        };
        match mutation {
            "slot" => *slot ^= 1,
            "session" => session_id.0[0] ^= 1,
            "candidate" => candidate[0] ^= 1,
            "duplicate signer" => signatures.push(signatures[0].clone()),
            "signature" => signatures[0].signature[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(changed.verify(&checkpoint, &fixture.head).is_err());
    }
}

#[test]
fn rejects_backward_links_with_forged_file_hashes() {
    let fixture = fixture();
    let head = VerifiedBlock::from_trusted(fixture.head);
    let mut proof: response::PartialBlockProof = reply(BACKWARD);
    let mut target = fixture.checkpoint;
    target.file_hash.0[0] ^= 1;
    proof.to.file_hash.0[0] ^= 1;
    let BlockLink::BlockLinkBack { to, .. } = &mut proof.steps[0] else {
        panic!("fixture must contain a backward link");
    };
    to.file_hash.0[0] ^= 1;
    assert!(proof.verify(&head, &target).is_err());
}

#[tokio::test]
async fn high_level_queries_verify_responses_from_an_adnl_server() {
    let fixture = fixture();
    let server = TestServer::start(move |bytes| {
        let request: Request = tl_proto::deserialize(&bytes).unwrap();
        match request {
            Request::GetMasterchainInfo => {
                tl_proto::serialize(Response::MasterchainInfo(response::MasterchainInfo {
                    last: reply::<response::AccountState>(ACCOUNT).id,
                    state_root_hash: Default::default(),
                    init: rston::tl::common::ZeroStateIdExt {
                        workchain: -1,
                        root_hash: Default::default(),
                        file_hash: Default::default(),
                    },
                }))
            }
            Request::GetBlockProof(_) => FORWARD.to_vec(),
            Request::GetAccountState(_) => ACCOUNT.to_vec(),
            Request::GetConfigParams(_) => CONFIG.to_vec(),
            Request::GetOneTransaction(_) => TRANSACTION.to_vec(),
            Request::RunSmcMethod(request) => {
                assert_eq!(request.mode, 7);
                assert_eq!(
                    request.method_id,
                    MethodId::from_name("active_election_id").value()
                );
                assert_eq!(
                    Boc::decode(request.params)
                        .unwrap()
                        .parse::<Tuple>()
                        .unwrap(),
                    Tuple::empty()
                );
                RUN_METHOD.to_vec()
            }
            _ => panic!("unexpected high-level request"),
        }
    })
    .await;
    let mut client = server.client().await;
    let checkpoint = VerifiedBlock::from_trusted(fixture.checkpoint);
    let head = client.get_masterchain_info(&checkpoint).await.unwrap();

    let account = client.get_account(&head, &fixture.elector).await.unwrap();
    assert_eq!(account.last_transaction_lt, fixture.elector_last_lt);
    client.get_config(&head, &[1, 4, 28, 34]).await.unwrap();
    let block = VerifiedBlock::from_trusted(fixture.transaction_block);
    client
        .get_transaction(&block, &fixture.elector, fixture.transaction_lt)
        .await
        .unwrap();
    let result = client
        .run_get_method(
            &head,
            &fixture.elector,
            "active_election_id",
            &Tuple::empty(),
        )
        .await
        .unwrap();
    assert_eq!(result.stack.len(), 1);
}

#[tokio::test]
async fn high_level_pool_queries_use_the_same_verifier() {
    let fixture = fixture();
    let server = TestServer::start(|_| ACCOUNT.to_vec()).await;
    let config = rston::global_config::GlobalConfig {
        liteservers: vec![server.config.clone()],
        ..Default::default()
    };
    let mut pool = rston::liteclient::LiteClientPool::new(config, Default::default()).unwrap();
    let block = VerifiedBlock::from_trusted(fixture.head);
    let account = pool.get_account(&block, &fixture.elector).await.unwrap();
    assert_eq!(account.last_transaction_lt, fixture.elector_last_lt);
}

#[tokio::test]
async fn block_sequence_numbers_preserve_all_unsigned_bits() {
    let fixture = fixture();
    let mut known = fixture.head;
    known.seqno = u32::MAX;
    let mut target = fixture.checkpoint;
    target.seqno = u32::MAX - 1;
    let server = TestServer::start(|bytes| {
        let Request::GetBlockProof(request) = tl_proto::deserialize(&bytes).unwrap() else {
            panic!("expected getBlockProof");
        };
        assert_eq!(request.known_block.seqno, -1);
        let target = request.target_block.unwrap();
        assert_eq!(target.seqno, -2);

        tl_proto::serialize(Response::PartialBlockProof(response::PartialBlockProof {
            complete: false,
            from: request.known_block,
            to: target,
            steps: Vec::new(),
        }))
    })
    .await;
    let mut client = server.client().await;
    let checkpoint = VerifiedBlock::from_trusted(known);
    let proof = client
        .get_block_proof(&checkpoint, &target, Default::default())
        .await
        .unwrap();
    assert_eq!(proof.from.seqno as u32, known.seqno);
    assert_eq!(proof.to.seqno as u32, target.seqno);
}

#[tokio::test]
async fn uninitialized_contract_error_matches_go() {
    let fixture = fixture();
    let server = TestServer::start(|_| {
        tl_proto::serialize(Response::Error(response::Error {
            code: -256,
            message: "contract is not initialized".into(),
        }))
    })
    .await;
    let mut client = server.client().await;
    let block = VerifiedBlock::from_trusted(fixture.head);
    let error = client
        .run_get_method(&block, &fixture.elector, "seqno", &Tuple::empty())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        LiteError::ContractExecution { exit_code: -256 }
    ));
}

fn external_message() -> Cell {
    let body = CellBuilder::build_from(0x12345678_u32).unwrap();
    let message = Message {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            src: None,
            dst: IntAddr::Std(StdAddr::ZERO),
            import_fee: Tokens::ZERO,
        }),
        init: None,
        body: body.as_slice().unwrap(),
        layout: None,
    };
    CellBuilder::build_from(message).unwrap()
}

#[tokio::test]
async fn message_submission_checks_kind_and_server_acknowledgement() {
    let sent = Arc::new(AtomicUsize::new(0));
    let observed = sent.clone();
    let message = external_message();
    let expected = message.clone();
    let server = TestServer::start(move |bytes| {
        let Request::SendMessage(request) = tl_proto::deserialize(&bytes).unwrap() else {
            panic!("expected sendMessage");
        };
        assert_eq!(Boc::decode(request.body).unwrap(), expected);
        observed.fetch_add(1, Ordering::SeqCst);
        tl_proto::serialize(Response::SendMsgStatus(response::SendMsgStatus {
            status: 1,
        }))
    })
    .await;
    let mut client = server.client().await;
    let hash = client.send_message(&message).await.unwrap();
    assert_eq!(hash, *message.repr_hash());
    assert_eq!(sent.load(Ordering::SeqCst), 1);

    let body = Cell::default();
    let internal = Message {
        info: MsgInfo::Int(IntMsgInfo::default()),
        init: None,
        body: body.as_slice().unwrap(),
        layout: None,
    };
    let internal = CellBuilder::build_from(internal).unwrap();
    assert!(client.send_message(&internal).await.is_err());
    assert_eq!(sent.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rejected_messages_are_not_retried() {
    let sent = Arc::new(AtomicUsize::new(0));
    let observed = sent.clone();
    let server = TestServer::start(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        tl_proto::serialize(Response::SendMsgStatus(response::SendMsgStatus {
            status: 0,
        }))
    })
    .await;
    let mut client = server.client().await;
    let error = client.send_message(&external_message()).await.unwrap_err();
    assert!(matches!(error, LiteError::MessageRejected { status: 0 }));
    assert_eq!(sent.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn proof_chain_deadline_covers_all_queries_and_preserves_the_checkpoint() {
    let fixture = fixture();
    let checkpoint = VerifiedBlock::from_trusted(fixture.ordinary_from);
    let mut target = fixture.ordinary_to;
    target.seqno = 3;
    let sent = Arc::new(AtomicUsize::new(0));
    let observed = sent.clone();
    let server = TestServer::with_delay(Duration::from_millis(35), move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        let mut part: response::PartialBlockProof = reply(ORDINARY);
        part.complete = false;
        tl_proto::serialize(Response::PartialBlockProof(part))
    })
    .await;
    let mut client = server.client().await;
    client.set_request_timeout(Duration::from_millis(60));
    let error = client
        .verify_proof_chain(&checkpoint, &target)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        LiteError::Timeout {
            operation: "verify_proof_chain",
            ..
        }
    ));
    assert_eq!(checkpoint.id(), &fixture.ordinary_from);
    tokio::time::sleep(Duration::from_millis(45)).await;
    assert_eq!(sent.load(Ordering::SeqCst), 2);
}
