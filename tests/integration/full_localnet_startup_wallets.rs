use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{TON_CONNECT_WALLETS_CONFIG, build_internal_message_boc};
use acton::wallets::{open_selected_wallets, prepare_localnet_wallets};
use acton_config::config::ActonConfig;
use acton_localnet::{CreateNetwork, catalog};
use anyhow::Result;
use expect_test::expect;
use serde_json::json;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton_api::Network;
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{EmulationResult, Executor, RunTransactionArgs};
use tycho_types::boc::BocRepr;
use tycho_types::cell::CellBuilder;
use tycho_types::models::{AccountState, IntAddr, ShardAccount, Transaction, TxInfo};

fn wallet_config() -> Result<ActonConfig> {
    let document: toml::Value = toml::from_str(TON_CONNECT_WALLETS_CONFIG)?;
    Ok(ActonConfig {
        wallets: Some(document["wallets"].clone().try_into()?),
        ..Default::default()
    })
}

#[test]
fn full_localnet_startup_wallets_accept_signed_transfers() -> Result<()> {
    let config = wallet_config()?;
    let names = config
        .wallets
        .as_ref()
        .unwrap()
        .wallets
        .keys()
        .filter(|name| {
            name.starts_with("wallet_v3")
                || name.starts_with("wallet_v4")
                || name.as_str() == "wallet_v5"
        })
        .cloned()
        .collect::<Vec<_>>();
    let wallets = open_selected_wallets(&config, &names, &Network::Localnet)?;
    let executor = Executor::new(ExecutorVerbosity::Off, None)?;
    let mut results = Vec::new();

    // Exercise the actual initial code/data with signature checks enabled. A
    // matching address alone would miss a wrong wallet ID or disabled signing.
    for prepared in prepare_localnet_wallets(&config, &names)? {
        let shard: ShardAccount = BocRepr::decode_hex(&prepared.shard_account_boc_hex)?;
        let account = shard.load_account()?.unwrap();
        let AccountState::Active(state) = &account.state else {
            panic!("wallet must be active")
        };
        let wallet = &wallets[&prepared.name];
        let address = wallet.address();
        let internal = build_internal_message_boc(address.clone(), address.clone(), 1_000_000);
        let message = wallet
            .wallet
            .create_ext_in_msg(vec![TonCell::from_boc(internal)?], 0, u32::MAX, false)?
            .to_boc_base64()?;
        let (result, _) = executor.run_transaction(
            &message,
            &RunTransactionArgs {
                shard_account: BocRepr::encode_base64(&shard)?,
                now: 1_800_000_000,
                lt: 1_000_000,
                ..Default::default()
            },
        )?;
        let EmulationResult::Success(result) = result else {
            panic!("signed transfer rejected: {result:?}")
        };
        let tx: Transaction = BocRepr::decode_base64(result.transaction.as_ref())?;
        let TxInfo::Ordinary(info) = tx.load_info()? else {
            panic!("ordinary wallet transaction")
        };
        results.push(json!({
            "name": prepared.name,
            "balance": account.balance.tokens.to_string(),
            "addressMatches": account.address == IntAddr::Std(address.clone()),
            "stateMatchesAddress": CellBuilder::build_from(state)?.repr_hash() == &address.address,
            "noPreviousTransaction": shard.last_trans_lt == 0,
            "transferAccepted": !info.aborted,
            "outMessages": tx.out_msg_count,
        }));
    }
    expect![[r#"
        [
          {
            "addressMatches": true,
            "balance": "100000000000",
            "name": "wallet_v3",
            "noPreviousTransaction": true,
            "outMessages": 1,
            "stateMatchesAddress": true,
            "transferAccepted": true
          },
          {
            "addressMatches": true,
            "balance": "100000000000",
            "name": "wallet_v3_r1",
            "noPreviousTransaction": true,
            "outMessages": 1,
            "stateMatchesAddress": true,
            "transferAccepted": true
          },
          {
            "addressMatches": true,
            "balance": "100000000000",
            "name": "wallet_v4",
            "noPreviousTransaction": true,
            "outMessages": 1,
            "stateMatchesAddress": true,
            "transferAccepted": true
          },
          {
            "addressMatches": true,
            "balance": "100000000000",
            "name": "wallet_v4_r1",
            "noPreviousTransaction": true,
            "outMessages": 1,
            "stateMatchesAddress": true,
            "transferAccepted": true
          },
          {
            "addressMatches": true,
            "balance": "100000000000",
            "name": "wallet_v5",
            "noPreviousTransaction": true,
            "outMessages": 1,
            "stateMatchesAddress": true,
            "transferAccepted": true
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&results)?);
    Ok(())
}

#[tokio::test]
async fn full_localnet_startup_wallets_persist_and_reject_collisions() -> Result<()> {
    let config = wallet_config()?;
    let names = vec!["wallet_v4".into(), "wallet_v5".into(), "wallet_v4".into()];
    let prepared = prepare_localnet_wallets(&config, &names)?;
    let root = tempfile::tempdir()?;
    catalog::create(
        root.path(),
        CreateNetwork {
            name: "wallets".into(),
            startup_wallets: prepared.clone(),
            ..Default::default()
        },
    )
    .await?;
    let reopened = catalog::list(root.path()).await?.remove(0);
    let mut errors = Vec::new();
    for request in [
        CreateNetwork {
            name: "import collision".into(),
            imported_account_bocs: vec![prepared[0].shard_account_boc_hex.clone()],
            startup_wallets: prepared.clone(),
            ..Default::default()
        },
        CreateNetwork {
            name: "invalid state".into(),
            startup_wallets: vec![acton_localnet::StartupWallet {
                name: "wallet".into(),
                shard_account_boc_hex: "00".into(),
            }],
            ..Default::default()
        },
    ] {
        errors.push(
            catalog::create(root.path(), request)
                .await
                .err()
                .expect("invalid genesis must fail")
                .to_string(),
        );
    }
    let mut aliases = config.clone();
    let wallets = &mut aliases.wallets.as_mut().unwrap().wallets;
    wallets.insert("alias".into(), wallets["wallet_v4"].clone());
    errors.push(
        prepare_localnet_wallets(&aliases, &["alias".into(), "wallet_v4".into()])
            .unwrap_err()
            .to_string(),
    );
    errors.push(
        prepare_localnet_wallets(&config, &["missing".into()])
            .unwrap_err()
            .to_string(),
    );
    aliases
        .wallets
        .as_mut()
        .unwrap()
        .wallets
        .get_mut("alias")
        .unwrap()
        .workchain = Some(-1);
    errors.push(
        prepare_localnet_wallets(&aliases, &["alias".into()])
            .unwrap_err()
            .to_string(),
    );

    expect![[r#"
        {
          "errors": [
            "Account 0:513ec97b0c602901c3cf14ac0aa588292468969cccd0d84a4c3fb81e7f897a9c was selected more than once for genesis",
            "Invalid genesis ShardAccount: invalid BOC",
            "Startup wallet 'wallet_v4' duplicates address 0:513ec97b0c602901c3cf14ac0aa588292468969cccd0d84a4c3fb81e7f897a9c",
            "Wallets are not found in Acton.toml: missing",
            "Startup wallet 'alias' must be in workchain 0 for Full localnet"
          ],
          "imports": 0,
          "names": [
            "wallet_v4",
            "wallet_v5"
          ],
          "networksAfterErrors": 1,
          "savedStateUnchanged": true
        }"#]].assert_eq(&serde_json::to_string_pretty(&json!({
        "names": reopened.network.config.startup_wallets.iter().map(|wallet| &wallet.name).collect::<Vec<_>>(),
        "savedStateUnchanged": serde_json::to_value(&reopened.network.config.startup_wallets)? == serde_json::to_value(prepared)?,
        "imports": reopened.network.config.imported_account_bocs.len(),
        "networksAfterErrors": catalog::list(root.path()).await?.len(),
        "errors": errors,
    }))?);
    Ok(())
}

#[tokio::test]
async fn full_localnet_cli_startup_wallets_use_project_defaults_and_explicit_selection()
-> Result<()> {
    let project = ProjectBuilder::new("full-localnet-wallets").build();
    std::fs::write(
        project.path().join("wallets.toml"),
        TON_CONNECT_WALLETS_CONFIG,
    )?;
    let path = project.path().join("Acton.toml");
    let mut config = std::fs::read_to_string(&path)?;
    config.push_str("\n[localnet]\naccounts = [\"wallet_v4\", \"wallet_v5\"]\n");
    std::fs::write(path, config)?;

    for args in [
        vec!["full-localnet", "create", "default"],
        vec![
            "full-localnet",
            "create",
            "explicit",
            "--accounts",
            "wallet_v1,wallet_v1",
        ],
    ] {
        project
            .acton()
            .current_dir(project.path())
            .args(args)
            .run()
            .success();
    }
    // A fresh client can read these definitions after wallet configuration is
    // gone; creation freezes public state instead of retaining a key dependency.
    std::fs::remove_file(project.path().join("wallets.toml"))?;
    project
        .acton()
        .current_dir(project.path())
        .args(["full-localnet", "list", "--json"])
        .run()
        .success();
    project
        .acton()
        .current_dir(project.path())
        .args([
            "full-localnet",
            "start",
            "default",
            "--accounts",
            "wallet_v1",
        ])
        .run()
        .failure()
        .assert_stderr_contains("Genesis options apply only to new networks");

    let networks = catalog::list(&project.path().join(".acton-localnet")).await?;
    let actual = networks.iter().map(|location| json!({
        "name": location.network.name,
        "wallets": location.network.config.startup_wallets.iter().map(|wallet| &wallet.name).collect::<Vec<_>>(),
    })).collect::<Vec<_>>();
    expect![[r#"
        [
          {
            "name": "default",
            "wallets": [
              "wallet_v4",
              "wallet_v5"
            ]
          },
          {
            "name": "explicit",
            "wallets": [
              "wallet_v1"
            ]
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);
    Ok(())
}
