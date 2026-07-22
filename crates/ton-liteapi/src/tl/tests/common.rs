use crate::tl::common::*;
use crate::tl::response::{
    BlockOutMsgQueueSize, BlockTransactionsExt, DispatchQueueInfo, DispatchQueueMessages,
    LibraryResultWithProof, LookupBlockResult, NonfinalCandidate, NonfinalPendingShardBlocks,
    NonfinalValidatorGroups, OutMsgQueueSizes, Response, ShardBlockProof,
};
use crate::tl::utils::FromResponse;
use crate::{liteclient::types::LiteError, tl::response::MasterchainInfo};
use std::str::FromStr;
use tl_proto::{deserialize, serialize};

#[test]
pub(super) fn test_int256_creation_and_display() {
    let int256 = Int256([1u8; 32]);
    let hex_str = int256.to_hex();
    assert_eq!(hex_str.len(), 64);

    // Test from_hex
    let parsed = Int256::from_hex(&hex_str).unwrap();
    assert_eq!(parsed, int256);
}

#[test]
pub(super) fn test_int256_random() {
    let int1 = Int256::random();
    let int2 = Int256::random();
    // Random values should be different (with extremely high probability)
    assert_ne!(int1, int2);
}

#[test]
pub(super) fn test_int256_from_str() {
    let hex_str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let int256 = Int256::from_str(hex_str).unwrap();
    assert_eq!(int256.to_hex(), hex_str);
}

#[test]
pub(super) fn test_int256_default() {
    let default_int = Int256::default();
    assert_eq!(default_int.0, [0u8; 32]);
}

#[test]
pub(super) fn test_block_id_creation() {
    let block_id = BlockId {
        workchain: -1,
        shard: 0x8000000000000000u64 as i64,
        seqno: 12345,
    };

    assert_eq!(block_id.workchain, -1);
    assert_eq!(block_id.shard, 0x8000000000000000u64 as i64);
    assert_eq!(block_id.seqno, 12345);
}

#[test]
pub(super) fn test_block_id_ext_creation() {
    let root_hash = Int256([1u8; 32]);
    let file_hash = Int256([2u8; 32]);

    let block_id_ext = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 1000,
        root_hash: root_hash.clone(),
        file_hash: file_hash.clone(),
    };

    assert_eq!(block_id_ext.workchain, 0);
    assert_eq!(block_id_ext.root_hash, root_hash);
    assert_eq!(block_id_ext.file_hash, file_hash);
}

#[test]
pub(super) fn test_block_id_ext_display() {
    let root_hash = Int256([0xABu8; 32]);
    let file_hash = Int256([0xCDu8; 32]);

    let block_id_ext = BlockIdExt {
        workchain: -1,
        shard: 0x8000000000000000u64 as i64,
        seqno: 100,
        root_hash,
        file_hash,
    };

    let display_str = format!("{}", block_id_ext);
    assert!(display_str.contains("-1"));
    assert!(display_str.contains("8000000000000000"));
    assert!(display_str.contains("100"));
}

#[test]
pub(super) fn test_account_id_creation() {
    let account_id = AccountId {
        workchain: 0,
        id: Int256([0x42u8; 32]),
    };

    assert_eq!(account_id.workchain, 0);
    assert_eq!(account_id.id.0[0], 0x42);
}

#[test]
pub(super) fn test_transaction_id3_creation() {
    let tx_id = TransactionId3 {
        account: Int256([0x11u8; 32]),
        lt: 123456789,
    };

    assert_eq!(tx_id.lt, 123456789);
    assert_eq!(tx_id.account.0[0], 0x11);
}

#[test]
pub(super) fn test_signature_creation() {
    let signature = Signature {
        node_id_short: Int256([0xAAu8; 32]),
        signature: vec![1, 2, 3, 4, 5],
    };

    assert_eq!(signature.signature.len(), 5);
    assert_eq!(signature.node_id_short.0[0], 0xAA);
}

#[test]
pub(super) fn test_signature_set_creation() {
    let sig1 = Signature {
        node_id_short: Int256([0x01u8; 32]),
        signature: vec![1, 2, 3],
    };

    let sig2 = Signature {
        node_id_short: Int256([0x02u8; 32]),
        signature: vec![4, 5, 6],
    };

    let sig_set = SignatureSet::Ordinary {
        validator_set_hash: 0x12345678,
        catchain_seqno: 42,
        signatures: vec![sig1, sig2],
    };

    match sig_set {
        SignatureSet::Ordinary {
            validator_set_hash,
            catchain_seqno,
            signatures,
        } => {
            assert_eq!(validator_set_hash, 0x12345678);
            assert_eq!(catchain_seqno, 42);
            assert_eq!(signatures.len(), 2);
        }
        _ => panic!("Wrong signature set variant"),
    }
}

#[test]
pub(super) fn test_signature_set_roundtrip_for_all_variants() {
    let ordinary = SignatureSet::Ordinary {
        validator_set_hash: 0x1234_5678,
        catchain_seqno: 42,
        signatures: vec![Signature {
            node_id_short: Int256([0xAB; 32]),
            signature: vec![1, 2, 3, 4],
        }],
    };
    let ordinary_encoded = serialize(&ordinary);
    let ordinary_decoded: SignatureSet =
        deserialize(&ordinary_encoded).expect("decode ordinary signature set");
    assert_eq!(ordinary_decoded, ordinary);

    let simplex = SignatureSet::Simplex {
        cc_seqno: 7,
        validator_set_hash: 0x89AB_CDEFu32 as i32,
        signatures: vec![Signature {
            node_id_short: Int256([0xCD; 32]),
            signature: vec![5, 6, 7],
        }],
        session_id: Int256([0xEF; 32]),
        slot: 3,
        candidate: vec![0xAA, 0xBB, 0xCC],
    };
    let simplex_encoded = serialize(&simplex);
    let simplex_decoded: SignatureSet =
        deserialize(&simplex_encoded).expect("decode simplex signature set");
    assert_eq!(simplex_decoded, simplex);
}

#[test]
pub(super) fn test_zero_state_id_ext_creation() {
    let zero_state = ZeroStateIdExt {
        workchain: -1,
        root_hash: Int256([0x33u8; 32]),
        file_hash: Int256([0x44u8; 32]),
    };

    assert_eq!(zero_state.workchain, -1);
    assert_eq!(zero_state.root_hash.0[0], 0x33);
    assert_eq!(zero_state.file_hash.0[0], 0x44);
}

#[test]
pub(super) fn test_transaction_id_with_all_fields() {
    let tx_id = TransactionId {
        mode: (),
        account: Some(Int256([0x55u8; 32])),
        lt: Some(999),
        hash: Some(Int256([0x66u8; 32])),
    };

    assert!(tx_id.account.is_some());
    assert!(tx_id.lt.is_some());
    assert!(tx_id.hash.is_some());
    assert_eq!(tx_id.lt.unwrap(), 999);
}

#[test]
pub(super) fn test_transaction_id_partial_fields() {
    let tx_id = TransactionId {
        mode: (),
        account: Some(Int256([0x77u8; 32])),
        lt: None,
        hash: None,
    };

    assert!(tx_id.account.is_some());
    assert!(tx_id.lt.is_none());
    assert!(tx_id.hash.is_none());
}

#[test]
pub(super) fn test_library_entry_creation() {
    let entry = LibraryEntry {
        hash: Int256([0x88u8; 32]),
        data: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };

    assert_eq!(entry.hash.0[0], 0x88);
    assert_eq!(entry.data.len(), 8);
}

#[test]
pub(super) fn test_string_creation_from_str() {
    let tl_string = String::from("Hello, World!");
    assert_eq!(format!("{}", tl_string), "Hello, World!");
}

#[test]
pub(super) fn test_string_creation_from_string() {
    let rust_string = std::string::String::from("Test String");
    let tl_string = String::new(rust_string);
    assert_eq!(format!("{}", tl_string), "Test String");
}

#[test]
pub(super) fn test_block_link_back_creation() {
    let from = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 100,
        root_hash: Int256([0x11u8; 32]),
        file_hash: Int256([0x22u8; 32]),
    };

    let to = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 101,
        root_hash: Int256([0x33u8; 32]),
        file_hash: Int256([0x44u8; 32]),
    };

    let link = BlockLink::BlockLinkBack {
        to_key_block: true,
        from: from.clone(),
        to: to.clone(),
        dest_proof: vec![1, 2, 3],
        proof: vec![4, 5, 6],
        state_proof: vec![7, 8, 9],
    };

    match link {
        BlockLink::BlockLinkBack {
            to_key_block,
            from: f,
            to: t,
            ..
        } => {
            assert!(to_key_block);
            assert_eq!(f.seqno, 100);
            assert_eq!(t.seqno, 101);
        }
        _ => panic!("Wrong variant"),
    }
}

pub(super) fn sample_int256(fill: u8) -> Int256 {
    Int256([fill; 32])
}

pub(super) fn sample_block_id_ext(seqno: i32) -> BlockIdExt {
    BlockIdExt {
        workchain: -1,
        shard: 0x8000_0000_0000_0000_u64 as i64,
        seqno,
        root_hash: sample_int256(1),
        file_hash: sample_int256(2),
    }
}

#[test]
pub(super) fn test_from_response_lookup_block_result() {
    let response = Response::LookupBlockResult(LookupBlockResult {
        id: sample_block_id_ext(1),
        mode: (),
        mc_block_id: sample_block_id_ext(2),
        client_mc_state_proof: vec![1],
        mc_block_proof: vec![2],
        shard_links: vec![],
        header: vec![3],
        prev_header: vec![4],
    });
    let parsed = LookupBlockResult::from_response(response).unwrap();
    assert_eq!(parsed.id.seqno, 1);

    let unexpected = LookupBlockResult::from_response(Response::MasterchainInfo(MasterchainInfo {
        last: sample_block_id_ext(10),
        state_root_hash: sample_int256(3),
        init: ZeroStateIdExt {
            workchain: -1,
            root_hash: sample_int256(4),
            file_hash: sample_int256(5),
        },
    }));
    assert!(matches!(unexpected, Err(LiteError::UnexpectedMessage)));
}

#[test]
pub(super) fn test_from_response_block_transactions_ext() {
    let response = Response::BlockTransactionsExt(BlockTransactionsExt {
        id: sample_block_id_ext(3),
        req_count: 1,
        incomplete: false,
        transactions: vec![1, 2],
        proof: vec![3, 4],
    });
    assert!(BlockTransactionsExt::from_response(response).is_ok());
    let unexpected =
        BlockTransactionsExt::from_response(Response::OutMsgQueueSizes(OutMsgQueueSizes {
            shards: vec![],
            ext_msg_queue_size_limit: 0,
        }));
    assert!(matches!(unexpected, Err(LiteError::UnexpectedMessage)));
}

#[test]
pub(super) fn test_from_response_library_result_with_proof() {
    let response = Response::LibraryResultWithProof(LibraryResultWithProof {
        id: sample_block_id_ext(4),
        mode: (),
        result: vec![],
        state_proof: vec![1],
        data_proof: vec![2],
    });
    assert!(LibraryResultWithProof::from_response(response).is_ok());
    let unexpected =
        LibraryResultWithProof::from_response(Response::ShardBlockProof(ShardBlockProof {
            masterchain_id: sample_block_id_ext(5),
            links: vec![],
        }));
    assert!(matches!(unexpected, Err(LiteError::UnexpectedMessage)));
}

#[test]
pub(super) fn test_from_response_queue_and_dispatch_types() {
    let queue =
        BlockOutMsgQueueSize::from_response(Response::BlockOutMsgQueueSize(BlockOutMsgQueueSize {
            mode: (),
            id: sample_block_id_ext(6),
            size: 10,
            proof: None,
        }))
        .unwrap();
    assert_eq!(queue.size, 10);

    let dispatch_info =
        DispatchQueueInfo::from_response(Response::DispatchQueueInfo(DispatchQueueInfo {
            mode: (),
            id: sample_block_id_ext(7),
            account_dispatch_queues: vec![],
            complete: true,
            proof: None,
        }))
        .unwrap();
    assert!(dispatch_info.complete);

    let dispatch_messages = DispatchQueueMessages::from_response(Response::DispatchQueueMessages(
        DispatchQueueMessages {
            mode: (),
            id: sample_block_id_ext(8),
            messages: vec![],
            complete: false,
            proof: None,
            messages_boc: None,
        },
    ))
    .unwrap();
    assert!(!dispatch_messages.complete);

    let unexpected = DispatchQueueMessages::from_response(Response::BlockOutMsgQueueSize(
        BlockOutMsgQueueSize {
            mode: (),
            id: sample_block_id_ext(9),
            size: 0,
            proof: None,
        },
    ));
    assert!(matches!(unexpected, Err(LiteError::UnexpectedMessage)));
}

#[test]
pub(super) fn test_from_response_nonfinal_types() {
    let candidate_id = NonfinalCandidateId {
        block_id: sample_block_id_ext(10),
        creator: sample_int256(9),
        collated_data_hash: sample_int256(8),
    };

    let candidate =
        NonfinalCandidate::from_response(Response::NonfinalCandidate(NonfinalCandidate {
            id: candidate_id.clone(),
            data: vec![1, 2, 3],
            collated_data: vec![4, 5, 6],
        }))
        .unwrap();
    assert_eq!(candidate.id.block_id.seqno, 10);

    let groups = NonfinalValidatorGroups::from_response(Response::NonfinalValidatorGroups(
        NonfinalValidatorGroups { groups: vec![] },
    ))
    .unwrap();
    assert!(groups.groups.is_empty());

    let pending = NonfinalPendingShardBlocks::from_response(Response::NonfinalPendingShardBlocks(
        NonfinalPendingShardBlocks {
            signed_blocks: vec![sample_block_id_ext(11)],
            candidates: vec![sample_block_id_ext(12)],
        },
    ))
    .unwrap();
    assert_eq!(pending.signed_blocks.len(), 1);
    assert_eq!(pending.candidates.len(), 1);

    let unexpected =
        NonfinalCandidate::from_response(Response::OutMsgQueueSizes(OutMsgQueueSizes {
            shards: vec![],
            ext_msg_queue_size_limit: 0,
        }));
    assert!(matches!(unexpected, Err(LiteError::UnexpectedMessage)));
}

#[test]
pub(super) fn test_block_link_forward_creation() {
    let from = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 200,
        root_hash: Int256([0x55u8; 32]),
        file_hash: Int256([0x66u8; 32]),
    };

    let to = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 201,
        root_hash: Int256([0x77u8; 32]),
        file_hash: Int256([0x88u8; 32]),
    };

    let sig_set = SignatureSet::Ordinary {
        validator_set_hash: 0xABCDEF,
        catchain_seqno: 5,
        signatures: vec![],
    };

    let link = BlockLink::BlockLinkForward {
        to_key_block: false,
        from: from.clone(),
        to: to.clone(),
        dest_proof: vec![10, 11],
        config_proof: vec![12, 13],
        signatures: sig_set,
    };

    match link {
        BlockLink::BlockLinkForward {
            to_key_block,
            signatures,
            ..
        } => {
            assert!(!to_key_block);
            match signatures {
                SignatureSet::Ordinary { catchain_seqno, .. } => assert_eq!(catchain_seqno, 5),
                _ => panic!("Wrong signature set variant"),
            }
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
pub(super) fn test_int256_equality() {
    let int1 = Int256([0xABu8; 32]);
    let int2 = Int256([0xABu8; 32]);
    let int3 = Int256([0xCDu8; 32]);

    assert_eq!(int1, int2);
    assert_ne!(int1, int3);
}

#[test]
pub(super) fn test_block_id_ext_equality() {
    let block1 = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 100,
        root_hash: Int256([0x11u8; 32]),
        file_hash: Int256([0x22u8; 32]),
    };

    let block2 = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 100,
        root_hash: Int256([0x11u8; 32]),
        file_hash: Int256([0x22u8; 32]),
    };

    let block3 = BlockIdExt {
        workchain: 0,
        shard: 0x8000000000000000u64 as i64,
        seqno: 101,
        root_hash: Int256([0x11u8; 32]),
        file_hash: Int256([0x22u8; 32]),
    };

    assert_eq!(block1, block2);
    assert_ne!(block1, block3);
}

#[test]
pub(super) fn test_int256_hash_consistency() {
    use std::collections::HashMap;

    let int1 = Int256([0x42u8; 32]);
    let int2 = Int256([0x42u8; 32]);

    let mut map = HashMap::new();
    map.insert(int1.clone(), "value1");
    map.insert(int2, "value2"); // Should replace value1

    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&int1), Some(&"value2"));
}

pub(super) fn to_hex(bytes: &[u8]) -> std::string::String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[test]
pub(super) fn request_roundtrip_all_constructors() {
    use crate::tl::request::*;

    fn i(fill: u8) -> Int256 {
        Int256([fill; 32])
    }
    fn b(seqno: i32, root_fill: u8, file_fill: u8) -> BlockIdExt {
        BlockIdExt {
            workchain: -1,
            shard: 0x8000_0000_0000_0000_u64 as i64,
            seqno,
            root_hash: i(root_fill),
            file_hash: i(file_fill),
        }
    }
    fn bid(seqno: i32) -> BlockId {
        BlockId {
            workchain: -1,
            shard: 0x8000_0000_0000_0000_u64 as i64,
            seqno,
        }
    }

    let cases = vec![
        Request::GetMasterchainInfo,
        Request::GetMasterchainInfoExt(GetMasterchainInfoExt { mode: 3 }),
        Request::GetTime,
        Request::GetVersion,
        Request::GetBlock(GetBlock { id: b(1, 1, 2) }),
        Request::GetState(GetState { id: b(2, 3, 4) }),
        Request::GetBlockHeader(GetBlockHeader {
            mode: (),
            id: b(3, 5, 6),
            with_state_update: Some(()),
            with_value_flow: None,
            with_extra: Some(()),
            with_shard_hashes: None,
            with_prev_blk_signatures: Some(()),
        }),
        Request::SendMessage(SendMessage {
            body: vec![1, 2, 3, 4, 5],
        }),
        Request::GetAccountState(GetAccountState {
            id: b(4, 7, 8),
            account: AccountId {
                workchain: 0,
                id: i(9),
            },
        }),
        Request::GetAccountStatePrunned(GetAccountState {
            id: b(5, 10, 11),
            account: AccountId {
                workchain: -1,
                id: i(12),
            },
        }),
        Request::RunSmcMethod(RunSmcMethod {
            mode: 5,
            id: b(6, 13, 14),
            account: AccountId {
                workchain: 0,
                id: i(15),
            },
            method_id: 99,
            params: vec![6, 7, 8],
        }),
        Request::GetShardInfo(GetShardInfo {
            id: b(7, 16, 17),
            workchain: -1,
            shard: 0x8000_0000_0000_0000,
            exact: true,
        }),
        Request::GetAllShardsInfo(GetAllShardsInfo { id: b(8, 18, 19) }),
        Request::GetOneTransaction(GetOneTransaction {
            id: b(9, 20, 21),
            account: AccountId {
                workchain: 0,
                id: i(22),
            },
            lt: 123,
        }),
        Request::GetTransactions(GetTransactions {
            count: 2,
            account: AccountId {
                workchain: 0,
                id: i(23),
            },
            lt: 456,
            hash: i(24),
        }),
        Request::LookupBlock(LookupBlock {
            mode: (),
            id: bid(10),
            seqno: Some(()),
            lt: Some(42),
            utime: Some(1000),
            with_state_update: Some(()),
            with_value_flow: None,
            with_extra: Some(()),
            with_shard_hashes: None,
            with_prev_blk_signatures: Some(()),
        }),
        Request::LookupBlockWithProof(LookupBlockWithProof {
            mode: (),
            id: bid(11),
            mc_block_id: b(12, 25, 26),
            lt: Some(43),
            utime: Some(1001),
        }),
        Request::ListBlockTransactions(ListBlockTransactions {
            id: b(13, 27, 28),
            mode: (),
            count: 5,
            after: Some(TransactionId3 {
                account: i(29),
                lt: 44,
            }),
            reverse_order: Some(()),
            want_proof: Some(()),
        }),
        Request::ListBlockTransactionsExt(ListBlockTransactions {
            id: b(14, 30, 31),
            mode: (),
            count: 0,
            after: None,
            reverse_order: None,
            want_proof: None,
        }),
        Request::GetBlockProof(GetBlockProof {
            mode: (),
            known_block: b(15, 32, 33),
            target_block: Some(b(16, 34, 35)),
            allow_weak_target: Some(()),
            base_block_from_request: Some(()),
        }),
        Request::GetConfigAll(GetConfigAll {
            mode: (),
            id: b(17, 36, 37),
            with_state_root: Some(()),
            with_libraries: None,
            with_state_extra_root: Some(()),
            with_shard_hashes: None,
            with_validator_set: Some(()),
            with_special_smc: None,
            with_accounts_root: Some(()),
            with_prev_blocks: None,
            with_workchain_info: Some(()),
            with_capabilities: None,
            extract_from_key_block: Some(()),
        }),
        Request::GetConfigParams(GetConfigParams {
            mode: (),
            id: b(18, 38, 39),
            param_list: vec![0, 1, 1000],
            with_state_root: Some(()),
            with_libraries: Some(()),
            with_state_extra_root: None,
            with_shard_hashes: None,
            with_validator_set: None,
            with_special_smc: None,
            with_accounts_root: None,
            with_prev_blocks: None,
            with_workchain_info: None,
            with_capabilities: Some(()),
            extract_from_key_block: None,
        }),
        Request::GetValidatorStats(GetValidatorStats {
            mode: (),
            id: b(19, 40, 41),
            limit: 64,
            start_after: Some(i(42)),
            modified_after: Some(777),
        }),
        Request::GetLibraries(GetLibraries {
            library_list: vec![i(43), i(44)],
        }),
        Request::GetLibrariesWithProof(GetLibrariesWithProof {
            id: b(20, 45, 46),
            mode: (),
            library_list: vec![],
        }),
        Request::GetShardBlockProof(GetShardBlockProof { id: b(21, 47, 48) }),
        Request::GetOutMsgQueueSizes(GetOutMsgQueueSizes {
            mode: (),
            wc: Some(-1),
            shard: Some(0x8000_0000_0000_0000),
        }),
        Request::GetBlockOutMsgQueueSize(GetBlockOutMsgQueueSize {
            mode: (),
            id: b(22, 49, 50),
            want_proof: Some(()),
        }),
        Request::GetDispatchQueueInfo(GetDispatchQueueInfo {
            mode: (),
            id: b(23, 51, 52),
            want_proof: Some(()),
            after_addr: Some(i(53)),
            max_accounts: 20,
        }),
        Request::GetDispatchQueueMessages(GetDispatchQueueMessages {
            mode: (),
            id: b(24, 54, 55),
            addr: i(56),
            after_lt: 1_234_567_890_123,
            max_messages: 17,
            want_proof: Some(()),
            one_account: None,
            message_boc: Some(()),
        }),
        Request::NonfinalGetValidatorGroups(NonfinalGetValidatorGroups {
            mode: (),
            wc: Some(-1),
            shard: Some(0x8000_0000_0000_0000),
        }),
        Request::NonfinalGetCandidate(NonfinalGetCandidate {
            id: NonfinalCandidateId {
                block_id: b(25, 57, 58),
                creator: i(59),
                collated_data_hash: i(60),
            },
        }),
        Request::NonfinalGetPendingShardBlocks(NonfinalGetPendingShardBlocks {
            mode: (),
            wc: Some(0),
            shard: Some(0x7000_0000_0000_0000),
        }),
    ];

    for case in cases {
        let encoded = serialize(&case);
        let decoded: Request = deserialize(&encoded).expect("request roundtrip decode");
        assert_eq!(decoded, case);
    }
}
