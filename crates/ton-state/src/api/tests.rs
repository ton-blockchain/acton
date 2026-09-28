pub(crate) mod fixture;

use anyhow::Result;
use expect_test::expect;
use rston::cell::{CellBuilder, HashBytes, Lazy};
use rston::models::{
    Account, AccountState, BlockId, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount,
    ShardIdent, StateInit, StdAddr,
};
use ton_node_db::{AccountSnapshot, ReadStats};

use super::account_info;

#[test]
fn account_lifecycle_uses_the_v2_wire_contract() -> Result<()> {
    let masterchain_block = BlockId {
        shard: ShardIdent::MASTERCHAIN,
        seqno: 42,
        root_hash: HashBytes([1; 32]),
        file_hash: HashBytes([2; 32]),
    };
    let code = CellBuilder::build_from(123_u32)?;
    let data = CellBuilder::build_from(456_u32)?;
    let mut rows = Vec::new();

    for state in [
        None,
        Some(AccountState::Uninit),
        Some(AccountState::Active(StateInit {
            code: Some(code),
            data: Some(data),
            ..Default::default()
        })),
        Some(AccountState::Frozen(HashBytes([3; 32]))),
    ] {
        let account = state
            .map(|state| {
                let mut balance = CurrencyCollection::new(1_234_567_890);
                balance
                    .other
                    .as_dict_mut()
                    .set(u32::MAX, rston::num::VarUint248::from(123_u32))?;

                anyhow::Ok(ShardAccount {
                    account: Lazy::new(&OptionalAccount(Some(Account {
                        address: IntAddr::Std(StdAddr::new(0, HashBytes([7; 32]))),
                        storage_stat: Default::default(),
                        last_trans_lt: 789,
                        balance,
                        state,
                    })))?,
                    last_trans_hash: HashBytes([4; 32]),
                    last_trans_lt: 777,
                })
            })
            .transpose()?;
        let snapshot = AccountSnapshot {
            masterchain_block,
            gen_utime: 1_700_000_000,
            shard_block: BlockId {
                shard: ShardIdent::BASECHAIN,
                ..masterchain_block
            },
            account,
            reads: ReadStats {
                records: 0,
                bytes: 0,
                cache_hits: 0,
            },
        };
        rows.push(account_info(snapshot)?);
    }

    expect![[r#"
        [
          {
            "@type": "raw.fullAccountState",
            "balance": "0",
            "extra_currencies": [],
            "last_transaction_id": {
              "@type": "internal.transactionId",
              "lt": "0",
              "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
            },
            "block_id": {
              "@type": "ton.blockIdExt",
              "workchain": -1,
              "shard": "-9223372036854775808",
              "seqno": 42,
              "root_hash": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
              "file_hash": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI="
            },
            "code": "",
            "data": "",
            "frozen_hash": "",
            "sync_utime": 1700000000,
            "state": "uninitialized",
            "suspended": false
          },
          {
            "@type": "raw.fullAccountState",
            "balance": "1234567890",
            "extra_currencies": [
              {
                "@type": "extraCurrency",
                "id": -1,
                "amount": "123"
              }
            ],
            "last_transaction_id": {
              "@type": "internal.transactionId",
              "lt": "777",
              "hash": "BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ="
            },
            "block_id": {
              "@type": "ton.blockIdExt",
              "workchain": -1,
              "shard": "-9223372036854775808",
              "seqno": 42,
              "root_hash": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
              "file_hash": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI="
            },
            "code": "",
            "data": "",
            "frozen_hash": "",
            "sync_utime": 1700000000,
            "state": "uninitialized",
            "suspended": false
          },
          {
            "@type": "raw.fullAccountState",
            "balance": "1234567890",
            "extra_currencies": [
              {
                "@type": "extraCurrency",
                "id": -1,
                "amount": "123"
              }
            ],
            "last_transaction_id": {
              "@type": "internal.transactionId",
              "lt": "777",
              "hash": "BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ="
            },
            "block_id": {
              "@type": "ton.blockIdExt",
              "workchain": -1,
              "shard": "-9223372036854775808",
              "seqno": 42,
              "root_hash": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
              "file_hash": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI="
            },
            "code": "te6ccgEBAQEABgAACAAAAHs=",
            "data": "te6ccgEBAQEABgAACAAAAcg=",
            "frozen_hash": "",
            "sync_utime": 1700000000,
            "state": "active",
            "suspended": false
          },
          {
            "@type": "raw.fullAccountState",
            "balance": "1234567890",
            "extra_currencies": [
              {
                "@type": "extraCurrency",
                "id": -1,
                "amount": "123"
              }
            ],
            "last_transaction_id": {
              "@type": "internal.transactionId",
              "lt": "777",
              "hash": "BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ="
            },
            "block_id": {
              "@type": "ton.blockIdExt",
              "workchain": -1,
              "shard": "-9223372036854775808",
              "seqno": 42,
              "root_hash": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
              "file_hash": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI="
            },
            "code": "",
            "data": "",
            "frozen_hash": "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=",
            "sync_utime": 1700000000,
            "state": "frozen",
            "suspended": false
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&rows)?);

    Ok(())
}
