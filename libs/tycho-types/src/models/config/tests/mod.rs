use super::*;
use crate::prelude::Boc;

#[test]
fn simple_config() {
    use std::num::NonZeroU32;

    let data = Boc::decode(include_bytes!("simple_config.boc")).unwrap();
    let blockchain_config = data.parse::<BlockchainConfig>().unwrap();

    assert_eq!(
        blockchain_config.get::<ConfigParam0>().unwrap(),
        Some(HashBytes([0x55; 32]))
    );
    assert_eq!(
        blockchain_config.get::<ConfigParam1>().unwrap(),
        Some(HashBytes([0x33; 32]))
    );
    assert_eq!(
        blockchain_config.get::<ConfigParam2>().unwrap(),
        Some(HashBytes([0x00; 32]))
    );
    assert_eq!(blockchain_config.get::<ConfigParam3>().unwrap(), None);
    assert_eq!(blockchain_config.get::<ConfigParam4>().unwrap(), None);

    assert!(blockchain_config.get::<ConfigParam6>().unwrap().is_none());

    assert!(blockchain_config.get::<ConfigParam7>().unwrap().is_some());

    assert_eq!(
        blockchain_config.get::<ConfigParam8>().unwrap(),
        Some(GlobalVersion {
            version: 35,
            capabilities: 0x717ae.into(),
        })
    );

    let mandatory_params = blockchain_config.get::<ConfigParam9>().unwrap().unwrap();
    for param in mandatory_params.keys() {
        param.unwrap();
    }

    let critical_params = blockchain_config.get::<ConfigParam10>().unwrap().unwrap();
    for param in critical_params.keys() {
        param.unwrap();
    }

    blockchain_config.get::<ConfigParam11>().unwrap().unwrap();

    let workchains = blockchain_config.get::<ConfigParam12>().unwrap().unwrap();
    for entry in workchains.iter() {
        let (id, descr) = entry.unwrap();
        println!("{id}: {descr:#?}");
    }

    assert!(blockchain_config.get::<ConfigParam13>().unwrap().is_none());

    let reward = blockchain_config.get::<ConfigParam14>().unwrap().unwrap();
    println!("{reward:#?}");

    let timings = blockchain_config.get::<ConfigParam15>().unwrap().unwrap();
    assert_eq!(timings, ElectionTimings {
        validators_elected_for: 900,
        elections_start_before: 450,
        elections_end_before: 50,
        stake_held_for: 450,
    });

    let validator_count = blockchain_config.get::<ConfigParam16>().unwrap().unwrap();
    assert_eq!(validator_count, ValidatorCountParams {
        max_validators: 1000,
        max_main_validators: 100,
        min_validators: 13,
    });

    let validator_stakes = blockchain_config.get::<ConfigParam17>().unwrap().unwrap();
    assert_eq!(validator_stakes, ValidatorStakeParams {
        min_stake: Tokens::new(10000000000000),
        max_stake: Tokens::new(10000000000000000),
        min_total_stake: Tokens::new(100000000000000),
        max_stake_factor: 3 << 16,
    });

    let storage_prices = blockchain_config.get::<ConfigParam18>().unwrap().unwrap();
    for entry in storage_prices.iter() {
        let (i, entry) = entry.unwrap();
        println!("{i}: {entry:#?}");
    }

    let mc_prices = blockchain_config.get::<ConfigParam20>().unwrap().unwrap();
    assert_eq!(mc_prices, GasLimitsPrices {
        gas_price: 655360000,
        gas_limit: 1000000,
        special_gas_limit: 100000000,
        gas_credit: 10000,
        block_gas_limit: 10000000,
        freeze_due_limit: 100000000,
        delete_due_limit: 1000000000,
        flat_gas_limit: 1000,
        flat_gas_price: 10000000,
    });

    let sc_prices = blockchain_config.get::<ConfigParam21>().unwrap().unwrap();
    assert_eq!(sc_prices, GasLimitsPrices {
        gas_price: 6553600,
        gas_limit: 1000000,
        special_gas_limit: 100000000,
        gas_credit: 10000,
        block_gas_limit: 10000000,
        freeze_due_limit: 100000000,
        delete_due_limit: 1000000000,
        flat_gas_limit: 1000,
        flat_gas_price: 1000000,
    });

    let mc_limits = blockchain_config.get::<ConfigParam22>().unwrap().unwrap();
    assert_eq!(mc_limits, BlockLimits {
        bytes: BlockParamLimits {
            underload: 131072,
            soft_limit: 524288,
            hard_limit: 1048576,
        },
        gas: BlockParamLimits {
            underload: 900000,
            soft_limit: 1200000,
            hard_limit: 2000000,
        },
        lt_delta: BlockParamLimits {
            underload: 1000,
            soft_limit: 5000,
            hard_limit: 10000,
        },
    });

    let sc_limits = blockchain_config.get::<ConfigParam23>().unwrap().unwrap();
    assert_eq!(sc_limits, mc_limits);

    let mc_msg_fwd_prices = blockchain_config.get::<ConfigParam24>().unwrap().unwrap();
    assert_eq!(mc_msg_fwd_prices, MsgForwardPrices {
        lump_price: 10000000,
        bit_price: 655360000,
        cell_price: 65536000000,
        ihr_price_factor: 98304,
        first_frac: 21845,
        next_frac: 21845,
    });

    let sc_msg_fwd_prices = blockchain_config.get::<ConfigParam25>().unwrap().unwrap();
    assert_eq!(sc_msg_fwd_prices, MsgForwardPrices {
        lump_price: 100000,
        bit_price: 6553600,
        cell_price: 655360000,
        ihr_price_factor: 98304,
        first_frac: 21845,
        next_frac: 21845,
    });

    let catchain_config = blockchain_config.get::<ConfigParam28>().unwrap().unwrap();
    assert_eq!(catchain_config, CatchainConfig {
        isolate_mc_validators: false,
        shuffle_mc_validators: true,
        mc_catchain_lifetime: 250,
        shard_catchain_lifetime: 250,
        shard_validators_lifetime: 1000,
        shard_validators_num: 11,
    });

    let consensus_config = blockchain_config.get::<ConfigParam29>().unwrap().unwrap();
    assert_eq!(consensus_config, ConsensusConfig {
        new_catchain_ids: true,
        round_candidates: NonZeroU32::new(3).unwrap(),
        next_candidate_delay_ms: 2000,
        consensus_timeout_ms: 16000,
        fast_attempts: 3,
        attempt_duration: 8,
        catchain_max_deps: 4,
        max_block_bytes: 2097152,
        max_collated_bytes: 2097152,
    });

    assert!(blockchain_config.get::<ConfigParam30>().unwrap().is_none());

    let fundamental_smc = blockchain_config.get::<ConfigParam31>().unwrap().unwrap();
    for entry in fundamental_smc.keys() {
        let address = entry.unwrap();
        println!("{address}");
    }

    assert!(blockchain_config.get::<ConfigParam32>().unwrap().is_none());
    assert!(blockchain_config.get::<ConfigParam33>().unwrap().is_none());

    let current_validator_set = blockchain_config.get::<ConfigParam34>().unwrap().unwrap();
    println!("current_vset: {current_validator_set:#?}");

    assert!(blockchain_config.get::<ConfigParam35>().unwrap().is_none());
    assert!(blockchain_config.get::<ConfigParam36>().unwrap().is_none());
    assert!(blockchain_config.get::<ConfigParam37>().unwrap().is_none());
}

#[test]
fn prod_config() {
    fn check_config(data: &[u8]) {
        let data = Boc::decode(data).unwrap();
        let config = data.parse::<BlockchainConfig>().unwrap();

        assert_eq!(config.get_elector_address().unwrap(), [0x33; 32]);
        assert_eq!(config.get_minter_address().unwrap(), [0x00; 32]);
        assert_eq!(config.get_fee_collector_address().unwrap(), [0x33; 32]);
        config.get_global_version().unwrap();

        let mandatory_params = config.get_mandatory_params().unwrap();
        let mandatory_params = mandatory_params
            .keys()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(mandatory_params, [
            0, 1, 9, 10, 12, 14, 15, 16, 17, 18, 20, 21, 22, 23, 24, 25, 28, 34
        ]);

        let critical_params = config.get_critical_params().unwrap();
        let critical_params = critical_params
            .keys()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(critical_params, [
            0, 1, 9, 10, 12, 14, 15, 16, 17, 32, 34, 36, 4294966295, 4294966296, 4294966297
        ]);

        let workchains = config.get_workchains().unwrap();
        for entry in workchains.iter() {
            entry.unwrap();
        }

        config.get_block_creation_rewards().unwrap();
        config.get_election_timings().unwrap();
        config.get_validator_count_params().unwrap();
        config.get_validator_stake_params().unwrap();

        let storage_prices = config.get_storage_prices().unwrap();
        for entry in storage_prices.iter() {
            entry.unwrap();
        }

        config.get_gas_prices(true).unwrap();
        config.get_gas_prices(false).unwrap();
        config.get_block_limits(true).unwrap();
        config.get_block_limits(false).unwrap();
        config.get_msg_forward_prices(true).unwrap();
        config.get_msg_forward_prices(false).unwrap();

        config.get_catchain_config().unwrap();

        config.get_consensus_config().unwrap();

        let fundamental_addresses = config.get_fundamental_addresses().unwrap();
        for entry in fundamental_addresses.keys() {
            entry.unwrap();
        }

        assert!(config.contains_prev_validator_set().unwrap());
        config.contains_next_validator_set().unwrap();

        config.get_current_validator_set().unwrap();
    }

    // Some old config from the network beginning
    check_config(include_bytes!("old_config.boc"));

    // Current config
    check_config(include_bytes!("new_config.boc"));
}

#[test]
fn create_config() {
    let mut config = BlockchainConfig::new_empty(HashBytes([0x55; 32]));

    let prices = GasLimitsPrices {
        gas_price: 123,
        gas_limit: 321,
        special_gas_limit: 234,
        gas_credit: 432,
        block_gas_limit: 345,
        freeze_due_limit: 543,
        delete_due_limit: 456,
        flat_gas_limit: 654,
        flat_gas_price: 567,
    };
    config.set_gas_prices(false, &prices).unwrap();
    assert_eq!(config.get_gas_prices(false).unwrap(), prices);

    config.set::<ConfigParam0>(&HashBytes([0x55; 32])).unwrap();
    assert_eq!(
        config.get::<ConfigParam0>().unwrap(),
        Some(HashBytes([0x55; 32]))
    );
}

#[test]
fn validator_subset() {
    use crate::boc::BocRepr;
    use crate::models::{ShardIdent, ShardStateUnsplit};

    let master_state =
        BocRepr::decode::<ShardStateUnsplit, _>(&include_bytes!("test_state_2_master.boc"))
            .unwrap();

    let mc_state_extra = master_state.load_custom().unwrap().unwrap();

    let new_session_seqno = mc_state_extra.validator_info.catchain_seqno;

    let cc_config = mc_state_extra.config.get_catchain_config().unwrap();

    let validator_set = mc_state_extra.config.get_current_validator_set().unwrap();

    let subset = validator_set
        .compute_subset(ShardIdent::new_full(0), &cc_config, new_session_seqno)
        .unwrap();

    let expected_list = vec![
        ValidatorDescription {
            public_key: "b3fe2cf2e598d9322cd61f7ae442c9318b8bcb6381d8f02f3af82743ce525e8c"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
        ValidatorDescription {
            public_key: "56d25914d4c907af41044c079b37cbdd4e41d1cf9d01d56c57797dac60778e70"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
        ValidatorDescription {
            public_key: "f62d4118e05d5fb57b37c17a4fc641a865c4133437cbfa35812cc5d1e6db85f1"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
        ValidatorDescription {
            public_key: "2bb9f8277868dfb00800ead2fbed23921a33c2696717f6e41c43a78d14984374"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
        ValidatorDescription {
            public_key: "45e7cc7a12af73baf6fd71041ce9ca09757a6dd7f7720eb683dde520dc19e90b"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
        ValidatorDescription {
            public_key: "dad309ba273ef26449a9824a31136e0cfcb904e0b37f99123b5ad8cc61ff3c82"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
        ValidatorDescription {
            public_key: "660c4725fd034e49b21f7a3980681da5cba999f24e7f9c7bd48ab8ca1e6c6477"
                .parse()
                .unwrap(),
            weight: 1,
            adnl_addr: None,
            mc_seqno_since: 0,
            prev_total_weight: 0,
        },
    ];
    let expected_hash_short = 730838627;

    assert_eq!(subset, (expected_list, expected_hash_short));
}

#[cfg(feature = "serde")]
#[test]
fn serde() {
    fn check_config(data: &[u8]) {
        let data = Boc::decode(data).unwrap();

        let original = data.parse::<BlockchainConfig>().unwrap();
        let json = serde_json::to_string_pretty(&original).unwrap();

        let parsed: BlockchainConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }

    // Some old config from the network beginning
    check_config(include_bytes!("old_config.boc"));

    // Current config
    check_config(include_bytes!("new_config.boc"));
}
