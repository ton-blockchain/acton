use super::*;
use crate::tl::common::*;
use tl_proto::{deserialize, serialize};

#[test]
fn response_roundtrip_all_constructors() {
    use crate::tl::response::*;

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
    fn zs(fill: u8) -> ZeroStateIdExt {
        ZeroStateIdExt {
            workchain: -1,
            root_hash: i(fill),
            file_hash: i(fill.wrapping_add(1)),
        }
    }

    let cases = vec![
        Response::MasterchainInfo(MasterchainInfo {
            last: b(1, 1, 2),
            state_root_hash: i(3),
            init: zs(4),
        }),
        Response::MasterchainInfoExt(MasterchainInfoExt {
            mode: (),
            version: 1,
            capabilities: 2,
            last: b(2, 5, 6),
            last_utime: 10,
            now: 11,
            state_root_hash: i(7),
            init: zs(8),
        }),
        Response::CurrentTime(CurrentTime { now: 123 }),
        Response::Version(Version {
            mode: 0,
            version: 1,
            capabilities: 2,
            now: 3,
        }),
        Response::BlockData(BlockData {
            id: b(3, 9, 10),
            data: vec![1, 2, 3],
        }),
        Response::BlockState(BlockState {
            id: b(4, 11, 12),
            root_hash: i(13),
            file_hash: i(14),
            data: vec![4, 5, 6, 7, 8],
        }),
        Response::BlockHeader(BlockHeader {
            id: b(5, 15, 16),
            mode: (),
            with_state_update: Some(()),
            with_value_flow: None,
            with_extra: Some(()),
            with_shard_hashes: None,
            with_prev_blk_signatures: Some(()),
            header_proof: vec![9, 10, 11],
        }),
        Response::SendMsgStatus(SendMsgStatus { status: 1 }),
        Response::AccountState(AccountState {
            id: b(6, 17, 18),
            shardblk: b(7, 19, 20),
            shard_proof: vec![1, 2, 3],
            proof: vec![4, 5],
            state: vec![6, 7, 8, 9, 10],
        }),
        Response::RunMethodResult(RunMethodResult {
            mode: (),
            id: b(8, 21, 22),
            shardblk: b(9, 23, 24),
            shard_proof: Some(vec![1, 2, 3]),
            proof: Some(vec![4, 5]),
            state_proof: Some(vec![6, 7, 8, 9, 10]),
            init_c7: Some(vec![11, 12]),
            lib_extras: Some(vec![13, 14, 15]),
            exit_code: -33,
            result: Some(vec![16, 17, 18, 19]),
        }),
        Response::ShardInfo(ShardInfo {
            id: b(10, 25, 26),
            shardblk: b(11, 27, 28),
            shard_proof: vec![1, 2, 3],
            shard_descr: vec![4, 5, 6],
        }),
        Response::AllShardsInfo(AllShardsInfo {
            id: b(12, 29, 30),
            proof: vec![1, 2, 3],
            data: vec![4, 5, 6, 7, 8],
        }),
        Response::TransactionInfo(TransactionInfo {
            id: b(13, 31, 32),
            proof: vec![1],
            transaction: vec![2, 3, 4],
        }),
        Response::TransactionList(TransactionList {
            ids: vec![b(14, 33, 34), b(15, 35, 36)],
            transactions: vec![1, 2, 3, 4, 5],
        }),
        Response::TransactionId(TransactionId {
            mode: (),
            account: Some(i(37)),
            lt: Some(44),
            hash: Some(i(38)),
            metadata: None,
        }),
        Response::BlockTransactions(BlockTransactions {
            id: b(16, 39, 40),
            req_count: 2,
            incomplete: false,
            ids: vec![TransactionId {
                mode: (),
                account: Some(i(41)),
                lt: Some(45),
                hash: None,
                metadata: None,
            }],
            proof: vec![9, 8, 7],
        }),
        Response::BlockTransactionsExt(BlockTransactionsExt {
            id: b(17, 42, 43),
            req_count: 3,
            incomplete: true,
            transactions: vec![6, 7, 8],
            proof: vec![5, 4, 3, 2, 1],
        }),
        Response::PartialBlockProof(PartialBlockProof {
            complete: true,
            from: b(18, 44, 45),
            to: b(19, 46, 47),
            steps: vec![BlockLink::BlockLinkForward {
                to_key_block: false,
                from: b(20, 48, 49),
                to: b(21, 50, 51),
                dest_proof: vec![1, 2, 3],
                config_proof: vec![4, 5],
                signatures: SignatureSet::Simplex {
                    cc_seqno: 1,
                    validator_set_hash: 2,
                    signatures: vec![Signature {
                        node_id_short: i(52),
                        signature: vec![6, 7, 8],
                    }],
                    session_id: i(53),
                    slot: 3,
                    candidate: vec![9, 10, 11, 12, 13],
                },
            }],
        }),
        Response::ConfigInfo(ConfigInfo {
            mode: (),
            id: b(22, 54, 55),
            state_proof: vec![1, 2, 3],
            config_proof: vec![4, 5],
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
        Response::ValidatorStats(ValidatorStats {
            mode: (),
            id: b(23, 56, 57),
            count: 7,
            complete: false,
            state_proof: vec![1, 2, 3],
            data_proof: vec![4, 5],
        }),
        Response::LibraryResult(LibraryResult {
            result: vec![LibraryEntry {
                hash: i(58),
                data: vec![1, 2, 3, 4, 5],
            }],
        }),
        Response::LibraryResultWithProof(LibraryResultWithProof {
            id: b(24, 59, 60),
            mode: (),
            result: vec![],
            state_proof: vec![1, 2, 3],
            data_proof: vec![4, 5],
        }),
        Response::ShardBlockProof(ShardBlockProof {
            masterchain_id: b(25, 61, 62),
            links: vec![ShardBlockLink {
                id: b(26, 63, 64),
                proof: vec![1, 2, 3],
            }],
        }),
        Response::LookupBlockResult(LookupBlockResult {
            id: b(27, 65, 66),
            mode: (),
            mc_block_id: b(28, 67, 68),
            client_mc_state_proof: vec![1],
            mc_block_proof: vec![2, 3, 4],
            shard_links: vec![],
            header: vec![5, 6, 7],
            prev_header: vec![8, 9],
        }),
        Response::OutMsgQueueSizes(OutMsgQueueSizes {
            shards: vec![OutMsgQueueSize {
                id: b(29, 69, 70),
                size: 88,
            }],
            ext_msg_queue_size_limit: 99,
        }),
        Response::BlockOutMsgQueueSize(BlockOutMsgQueueSize {
            mode: (),
            id: b(30, 71, 72),
            size: 100,
            proof: Some(vec![1, 2, 3]),
        }),
        Response::DispatchQueueInfo(DispatchQueueInfo {
            mode: (),
            id: b(31, 73, 74),
            account_dispatch_queues: vec![AccountDispatchQueueInfo {
                addr: i(75),
                size: 1,
                min_lt: 2,
                max_lt: 3,
            }],
            complete: true,
            proof: Some(vec![1, 2, 3]),
        }),
        Response::DispatchQueueMessages(DispatchQueueMessages {
            mode: (),
            id: b(32, 76, 77),
            messages: vec![],
            complete: false,
            proof: Some(vec![1, 2]),
            messages_boc: Some(vec![3, 4, 5]),
        }),
        Response::NonfinalCandidate(NonfinalCandidate {
            id: NonfinalCandidateId {
                block_id: b(33, 78, 79),
                creator: i(80),
                collated_data_hash: i(81),
            },
            data: vec![1, 2, 3],
            collated_data: vec![4, 5, 6, 7, 8],
        }),
        Response::NonfinalValidatorGroups(NonfinalValidatorGroups {
            groups: vec![NonfinalValidatorGroupInfo {
                next_block_id: BlockId {
                    workchain: -1,
                    shard: 0x8000_0000_0000_0000_u64 as i64,
                    seqno: 34,
                },
                cc_seqno: 9,
                prev: vec![b(34, 82, 83)],
                candidates: vec![NonfinalCandidateInfo {
                    id: NonfinalCandidateId {
                        block_id: b(35, 84, 85),
                        creator: i(86),
                        collated_data_hash: i(87),
                    },
                    available: true,
                    approved_weight: 1,
                    signed_weight: 2,
                    total_weight: 3,
                }],
            }],
        }),
        Response::NonfinalPendingShardBlocks(NonfinalPendingShardBlocks {
            signed_blocks: vec![b(36, 88, 89)],
            candidates: vec![b(37, 90, 91)],
        }),
        Response::Error(Error {
            code: -400,
            message: String::from("tl-error"),
        }),
    ];

    for case in cases {
        let encoded = serialize(&case);
        let decoded: Response = deserialize(&encoded).expect("response roundtrip decode");
        assert_eq!(decoded, case);
    }
}

fn from_hex(input: &str) -> Vec<u8> {
    assert_eq!(input.len() % 2, 0, "hex string must have even length");
    (0..input.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&input[idx..idx + 2], 16).expect("valid hex byte"))
        .collect()
}

#[test]
fn golden_fixtures_high_risk_constructors() {
    use crate::tl::request::*;
    use crate::tl::response::*;

    const GET_CONFIG_ALL_HEX: &str = "b7261b9155810000ffffffff00000000000000800a00000011111111111111111111111111111111111111111111111111111111111111112222222222222222222222222222222222222222222222222222222222222222";
    const GET_CONFIG_PARAMS_HEX: &str = "191c112a03020000ffffffff00000000000000800b00000033333333333333333333333333333333333333333333333333333333333333334444444444444444444444444444444444444444444444444444444444444444030000000000000001000000e8030000";
    const GET_DISPATCH_QUEUE_MESSAGES_HEX: &str = "3964fdbb05000000ffffffff00000000000000800c000000555555555555555555555555555555555555555555555555555555555555555566666666666666666666666666666666666666666666666666666666666666667777777777777777777777777777777777777777777777777777777777777777cb04fb711f01000011000000";
    const LOOKUP_BLOCK_WITH_PROOF_HEX: &str = "f85f049c06000000ffffffff00000000000000800d000000ffffffff00000000000000800e000000888888888888888888888888888888888888888888888888888888888888888899999999999999999999999999999999999999999999999999999999999999992a0000000000000000f15365";
    const RUN_METHOD_RESULT_HEX: &str = "6b619aa31f000000ffffffff000000000000008014000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbffffffff000000000000008015000000ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd030102030204050005060708090a0000020b0c00030d0e0fdfffffff0410111213000000";
    const SIGNATURE_SET_SIMPLEX_HEX: &str = "009824ac070000004433221101000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee03010203ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff090000000504050607080000";

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

    let get_config_all = Request::GetConfigAll(GetConfigAll {
        mode: (),
        id: b(10, 0x11, 0x22),
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
    });
    assert_eq!(to_hex(&serialize(&get_config_all)), GET_CONFIG_ALL_HEX);
    assert_eq!(
        deserialize::<Request>(&from_hex(GET_CONFIG_ALL_HEX)).expect("decode getConfigAll fixture"),
        get_config_all
    );

    let get_config_params = Request::GetConfigParams(GetConfigParams {
        mode: (),
        id: b(11, 0x33, 0x44),
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
    });
    assert_eq!(
        to_hex(&serialize(&get_config_params)),
        GET_CONFIG_PARAMS_HEX
    );
    assert_eq!(
        deserialize::<Request>(&from_hex(GET_CONFIG_PARAMS_HEX))
            .expect("decode getConfigParams fixture"),
        get_config_params
    );

    let get_dispatch_queue_messages = Request::GetDispatchQueueMessages(GetDispatchQueueMessages {
        mode: (),
        id: b(12, 0x55, 0x66),
        addr: i(0x77),
        after_lt: 1_234_567_890_123,
        max_messages: 17,
        want_proof: Some(()),
        one_account: None,
        message_boc: Some(()),
    });
    assert_eq!(
        to_hex(&serialize(&get_dispatch_queue_messages)),
        GET_DISPATCH_QUEUE_MESSAGES_HEX
    );
    assert_eq!(
        deserialize::<Request>(&from_hex(GET_DISPATCH_QUEUE_MESSAGES_HEX))
            .expect("decode getDispatchQueueMessages fixture"),
        get_dispatch_queue_messages
    );

    let lookup_block_with_proof = Request::LookupBlockWithProof(LookupBlockWithProof {
        mode: (),
        id: bid(13),
        mc_block_id: b(14, 0x88, 0x99),
        lt: Some(42),
        utime: Some(1_700_000_000),
    });
    assert_eq!(
        to_hex(&serialize(&lookup_block_with_proof)),
        LOOKUP_BLOCK_WITH_PROOF_HEX
    );
    assert_eq!(
        deserialize::<Request>(&from_hex(LOOKUP_BLOCK_WITH_PROOF_HEX))
            .expect("decode lookupBlockWithProof fixture"),
        lookup_block_with_proof
    );

    let run_method_result = Response::RunMethodResult(RunMethodResult {
        mode: (),
        id: b(20, 0xaa, 0xbb),
        shardblk: b(21, 0xcc, 0xdd),
        shard_proof: Some(vec![1, 2, 3]),
        proof: Some(vec![4, 5]),
        state_proof: Some(vec![6, 7, 8, 9, 10]),
        init_c7: Some(vec![11, 12]),
        lib_extras: Some(vec![13, 14, 15]),
        exit_code: -33,
        result: Some(vec![16, 17, 18, 19]),
    });
    assert_eq!(
        to_hex(&serialize(&run_method_result)),
        RUN_METHOD_RESULT_HEX
    );
    assert_eq!(
        deserialize::<Response>(&from_hex(RUN_METHOD_RESULT_HEX))
            .expect("decode runMethodResult fixture"),
        run_method_result
    );

    let simplex = SignatureSet::Simplex {
        cc_seqno: 7,
        validator_set_hash: 0x1122_3344,
        signatures: vec![Signature {
            node_id_short: i(0xee),
            signature: vec![1, 2, 3],
        }],
        session_id: i(0xff),
        slot: 9,
        candidate: vec![4, 5, 6, 7, 8],
    };
    assert_eq!(to_hex(&serialize(&simplex)), SIGNATURE_SET_SIMPLEX_HEX);
    assert_eq!(
        deserialize::<SignatureSet>(&from_hex(SIGNATURE_SET_SIMPLEX_HEX))
            .expect("decode simplex signature set fixture"),
        simplex
    );
}
