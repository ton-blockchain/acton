//! Native traffic generation using shared wallet code and Acton's contract templates.
//! Every scenario owns fresh keys; no user wallet or faucet key leaves its owner.

use super::{
    ActivityConfig, ActivityOutcome, ActivityRun, ActivityScenario, ActivityWalletVersion,
    contracts,
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ed25519_dalek::SigningKey;
use rand::Rng;
use serde_json::{Value, json};
use std::{
    str::FromStr,
    time::{Duration, Instant},
};
use tokio::sync::watch;
use ton::{
    ton_core::{cell::TonCell, traits::tlb::TLB},
    ton_wallet::{KeyPair, TonWallet, WalletVersion},
};
use tycho_types::{
    boc::{Boc, BocRepr},
    cell::Cell,
    models::{
        CurrencyCollection, IntAddr, OwnedRelaxedMessage, RelaxedIntMsgInfo, RelaxedMsgInfo,
        StateInit, StdAddr, Transaction, TxInfo,
    },
};

use contracts::GRAM;

#[derive(Clone)]
/// Shares network connections while each worker owns its signing material.
pub(crate) struct Engine {
    client: reqwest::Client,
    endpoints: crate::Endpoints,
}

struct Sender {
    wallet: TonWallet,
    address: StdAddr,
    seqno: u32,
}

struct Transfer {
    destination: StdAddr,
    amount: u128,
    body: Cell,
    init: Option<StateInit>,
    contract: bool,
}

/// Owns one scenario's keys and progress from funding through cancellation.
/// Keys live only in memory and are never included in the persisted history.
pub(crate) struct Scenario {
    sender: Option<Sender>,
    progress: ActivityRun,
    started: Instant,
}

impl Scenario {
    pub(crate) fn new(run: ActivityRun) -> Self {
        Self {
            sender: None,
            progress: run,
            started: Instant::now(),
        }
    }

    /// Finalizes the same progress record on success, failure or cancellation,
    /// including confirmations collected before a worker was interrupted.
    pub(crate) fn finish(self, result: Option<Result<()>>, target: &str) -> ActivityRun {
        let mut run = self.progress;
        run.duration_ms = self.started.elapsed().as_millis() as u64;

        match result {
            Some(Ok(())) => run.outcome = ActivityOutcome::Completed,
            Some(Err(error)) => {
                run.outcome = ActivityOutcome::Failed;
                run.error = Some(format!("{error:#}"));
            }
            None => {}
        }

        log::info!(
            "operation=activity_scenario target={target} scenario={:?} id={} duration_ms={} outcome={:?} confirmed={} error={}",
            run.scenario,
            run.id,
            run.duration_ms,
            run.outcome,
            run.confirmed_messages,
            run.error.as_deref().unwrap_or("")
        );
        run
    }
}

impl Engine {
    /// Shares HTTP connections across a run; every signing wallet remains private
    /// to a single funding group or scenario, so seqnos cannot race.
    pub(crate) fn new(endpoints: crate::Endpoints) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .connect_timeout(Duration::from_secs(3))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            endpoints,
        })
    }

    /// Uses one faucet request per launch, then a V5 wallet distributes funds in
    /// batches. Fresh scenario wallets can subsequently submit in parallel instead
    /// of serializing every scenario through the network's single faucet wallet.
    pub(crate) async fn fund(
        &self,
        scenarios: &mut [Scenario],
        config: &ActivityConfig,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<bool> {
        if *cancel.borrow() {
            return Ok(false);
        }

        let mut transfers = Vec::with_capacity(scenarios.len());

        for scenario in scenarios {
            let progress = &mut scenario.progress;
            let version = if progress.scenario == ActivityScenario::Batches {
                ActivityWalletVersion::V5r1
            } else {
                config.wallet_versions
                    [rand::thread_rng().gen_range(0..config.wallet_versions.len())]
            };
            let sender = Sender::new(version)?;
            let count = if progress.scenario == ActivityScenario::Batches {
                let size = if config.randomize_batch_size {
                    rand::thread_rng().gen_range(2..=config.max_batch_size)
                } else {
                    config.max_batch_size
                };
                progress.batch_size = Some(size);
                size
            } else {
                1
            };

            transfers.push(Transfer {
                destination: sender.address.clone(),
                amount: u128::from(config.transfer_amount) * u128::from(count) + 20 * GRAM,
                body: Cell::default(),
                init: None,
                contract: false,
            });
            progress.address = Some(sender.address.to_string());
            scenario.sender = Some(sender);
        }

        let mut distributor = Sender::new(ActivityWalletVersion::V5r1)?;
        let amount = transfers
            .iter()
            .map(|transfer| transfer.amount)
            .sum::<u128>()
            + 20 * GRAM;
        let funded = self
            .post(
                &format!("{}/acton_fundAccount", self.endpoints.admin),
                json!({"address": distributor.address.to_string(), "amount": amount}),
            )
            .await;

        // The faucet signs with a shared seqno. Drain this request before Stop
        // completes so a subsequent run cannot overtake an in-flight submission.
        if *cancel.borrow() {
            return Ok(false);
        }
        let funded = funded.context("Could not fund the activity wallets")?;
        let hash = funded["hash"]
            .as_str()
            .context("Faucet returned no message hash")?;
        tokio::select! {
            biased;
            _ = cancel.changed() => return Ok(false),
            result = self.confirm(&distributor.address, hash, false) => {
                result?;
            }
        }

        // Stay below the V5 action limit even for the largest configured launch.
        let mut transfers = transfers.into_iter();
        loop {
            let batch: Vec<_> = transfers.by_ref().take(128).collect();
            if batch.is_empty() {
                break;
            }
            if *cancel.borrow() {
                return Ok(false);
            }
            tokio::select! {
                biased;
                _ = cancel.changed() => return Ok(false),
                result = self.send(&mut distributor, batch, None) => result?,
            }
        }
        Ok(true)
    }

    /// Executes a funded scenario and records only confirmed direct submissions.
    /// The caller can cancel this future without losing its partial progress.
    pub(crate) async fn run(&self, config: &ActivityConfig, work: &mut Scenario) -> Result<()> {
        let scenario = work.progress.scenario;
        let sender = work
            .sender
            .as_mut()
            .context("Activity wallet was not funded")?;
        let progress = &mut work.progress;
        let recipient = Sender::new(ActivityWalletVersion::V4r2)?.address;
        let transfers = progress.batch_size.unwrap_or(1);

        match scenario {
            ActivityScenario::Transfers | ActivityScenario::Batches => {
                let mut messages = Vec::with_capacity(transfers.into());
                for _ in 0..transfers {
                    messages.push(Transfer {
                        destination: Sender::new(ActivityWalletVersion::V4r2)?.address,
                        amount: config.transfer_amount.into(),
                        body: contracts::comment("Acton activity")?,
                        init: None,
                        contract: false,
                    });
                }
                self.send(sender, messages, Some(progress)).await?;
            }
            ActivityScenario::Jettons => {
                let (minter, init) = contracts::jetton(&sender.address)?;
                let mint = contracts::mint_jettons(&sender.address)?;
                self.send_contract(sender, &minter, mint, Some(init), progress)
                    .await?;
                let wallet = self.jetton_wallet(&minter, &sender.address).await?;
                self.wait_active(&wallet).await?;

                let transfer = contracts::transfer_jettons(&sender.address, &recipient)?;
                self.send_contract(sender, &wallet, transfer, None, progress)
                    .await?;
                let recipient_wallet = self.jetton_wallet(&minter, &recipient).await?;
                self.wait_active(&recipient_wallet).await?;
                self.wait_supply(&recipient_wallet, "get_wallet_data", 100 * GRAM)
                    .await?;

                let burn = contracts::burn_jettons(&sender.address)?;
                self.send_contract(sender, &wallet, burn, None, progress)
                    .await?;
                self.wait_supply(&minter, "get_jetton_data", 950 * GRAM)
                    .await?;
            }
            ActivityScenario::Nfts => {
                let (collection, init) = contracts::collection(&sender.address)?;
                let mint = contracts::mint_nft(&sender.address)?;
                self.send_contract(sender, &collection, mint, Some(init), progress)
                    .await?;
                let stack = self
                    .getter(
                        &collection,
                        "get_nft_address_by_index",
                        json!([["num", "0"]]),
                    )
                    .await?;
                let item = stack_address(&stack, 0)?;
                self.wait_active(&item).await?;

                let transfer = contracts::transfer_nft(&sender.address, &recipient)?;
                self.send_contract(sender, &item, transfer, None, progress)
                    .await?;
                let owner = self.getter(&item, "get_nft_data", json!([])).await?;
                ensure!(
                    stack_address(&owner, 3)? == recipient,
                    "NFT ownership did not change after the transfer"
                );
            }
        }
        Ok(())
    }
}

impl Sender {
    fn new(version: ActivityWalletVersion) -> Result<Self> {
        let secret: [u8; 32] = rand::random();
        let key = SigningKey::from_bytes(&secret);
        let wallet = TonWallet::new(
            match version {
                ActivityWalletVersion::V3r2 => WalletVersion::V3R2,
                ActivityWalletVersion::V4r2 => WalletVersion::V4R2,
                ActivityWalletVersion::V5r1 => WalletVersion::V5R1,
            },
            KeyPair {
                public_key: key.verifying_key().to_bytes(),
                secret_key: key.to_keypair_bytes(),
            },
        )?;
        let address = StdAddr::from_str(&wallet.address.to_hex())?;
        Ok(Self {
            wallet,
            address,
            seqno: 0,
        })
    }
}

impl Engine {
    async fn post(&self, url: &str, body: Value) -> Result<Value> {
        let response = self.client.post(url).json(&body).send().await?;
        Self::response(response).await
    }

    async fn get(&self, method: &str, address: &StdAddr) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/{method}", self.endpoints.api_v2))
            .query(&[("address", address.to_string()), ("limit", "32".to_owned())])
            .send()
            .await?;
        Self::response(response).await
    }

    async fn response(response: reqwest::Response) -> Result<Value> {
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .context("Localnet returned invalid JSON")?;
        ensure!(
            status.is_success() && value["ok"] == true,
            "Localnet request failed ({status}): {}",
            value["error"].as_str().unwrap_or("No error details")
        );
        value
            .get("result")
            .cloned()
            .context("Localnet returned no result")
    }

    async fn getter(&self, address: &StdAddr, method: &str, stack: Value) -> Result<Value> {
        let result = self
            .post(
                &format!("{}/runGetMethod", self.endpoints.api_v2),
                json!({"address": address.to_string(), "method": method, "stack": stack}),
            )
            .await?;
        ensure!(
            result["exit_code"] == 0,
            "Getter {method} failed for {address}"
        );
        Ok(result["stack"].clone())
    }

    async fn jetton_wallet(&self, minter: &StdAddr, owner: &StdAddr) -> Result<StdAddr> {
        let owner = STANDARD.encode(Boc::encode(contracts::build(owner)?));
        let stack = self
            .getter(minter, "get_wallet_address", json!([["tvm.Slice", owner]]))
            .await?;
        stack_address(&stack, 0)
    }

    async fn wait_active(&self, address: &StdAddr) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                if self.get("getAddressInformation", address).await?["state"] == "active" {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .with_context(|| format!("Contract {address} did not become active within 60 seconds"))?
    }

    /// A wallet's successful transaction alone does not confirm its downstream
    /// Jetton message. Wait for the recipient balance or minter supply to change.
    async fn wait_supply(&self, address: &StdAddr, method: &str, expected: u128) -> Result<()> {
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let stack = self.getter(address, method, json!([])).await?;
                let number = stack[0][1]
                    .as_str()
                    .context("Getter returned no token amount")?;
                let value = if let Some(hex) = number.strip_prefix("0x") {
                    u128::from_str_radix(hex, 16)?
                } else {
                    number.parse::<u128>()?
                };
                if value == expected {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .with_context(|| {
            format!("Token amount at {address} did not reach {expected} within 60 seconds")
        })?
    }

    async fn confirm(&self, address: &StdAddr, hash: &str, contract: bool) -> Result<Value> {
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                let transactions = self.get("getTransactions", address).await?;
                if let Some(transaction) = transactions
                    .as_array()
                    .context("Invalid transaction list")?
                    .iter()
                    .find(|tx| tx["in_msg"]["hash"] == hash)
                {
                    if contract {
                        let data = transaction["data"]
                            .as_str()
                            .context("Transaction has no BoC")?;
                        let tx: Transaction = BocRepr::decode(STANDARD.decode(data)?)?;
                        ensure!(
                            !matches!(tx.load_info()?, TxInfo::Ordinary(info) if info.aborted),
                            "Transaction {hash} was aborted at {address}"
                        );
                    }
                    return Ok(transaction.clone());
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .with_context(|| {
            format!("Message {hash} was not confirmed at {address} within 90 seconds")
        })?
    }

    async fn send_contract(
        &self,
        sender: &mut Sender,
        destination: &StdAddr,
        body: Cell,
        init: Option<StateInit>,
        progress: &mut ActivityRun,
    ) -> Result<()> {
        self.send(
            sender,
            vec![Transfer {
                destination: destination.clone(),
                amount: 2 * GRAM,
                body,
                init,
                contract: true,
            }],
            Some(progress),
        )
        .await
    }

    async fn send(
        &self,
        sender: &mut Sender,
        transfers: Vec<Transfer>,
        mut progress: Option<&mut ActivityRun>,
    ) -> Result<()> {
        let messages = transfers
            .iter()
            .map(|transfer| {
                let message = OwnedRelaxedMessage {
                    info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
                        bounce: transfer.contract && transfer.init.is_none(),
                        dst: IntAddr::Std(transfer.destination.clone()),
                        value: CurrencyCollection::new(transfer.amount),
                        ..Default::default()
                    }),
                    init: transfer.init.clone(),
                    body: transfer.body.clone().into(),
                    layout: None,
                };
                Ok(TonCell::from_boc(BocRepr::encode(message)?)?)
            })
            .collect::<Result<Vec<_>>>()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as u32;
        let boc = sender
            .wallet
            .create_ext_in_msg(
                messages,
                sender.seqno,
                now.saturating_add(90),
                sender.seqno == 0,
            )?
            .to_boc()?;
        let result = self
            .post(
                &format!("{}/sendBocReturnHash", self.endpoints.api_v2),
                json!({"boc": STANDARD.encode(boc)}),
            )
            .await?;
        let hash = result["hash"]
            .as_str()
            .context("Submission returned no message hash")?;
        let transaction = self.confirm(&sender.address, hash, true).await?;
        sender.seqno += 1;

        let outgoing = transaction["out_msgs"]
            .as_array()
            .context("Transaction has no outgoing messages")?;
        ensure!(
            outgoing.len() == transfers.len(),
            "Wallet produced {} of {} requested transfers",
            outgoing.len(),
            transfers.len()
        );
        for (message, transfer) in outgoing.iter().zip(&transfers) {
            let hash = message["hash"]
                .as_str()
                .context("Outgoing message has no hash")?;
            self.confirm(&transfer.destination, hash, transfer.contract)
                .await?;
            if let Some(progress) = progress.as_deref_mut() {
                progress.confirmed_messages += 1;
            }
        }
        Ok(())
    }
}

fn stack_address(stack: &Value, index: usize) -> Result<StdAddr> {
    let cell = stack[index][1]["bytes"]
        .as_str()
        .context("Getter returned no address cell")?;
    let address: IntAddr = Boc::decode(STANDARD.decode(cell)?)?.parse()?;
    match address {
        IntAddr::Std(address) => Ok(address),
        IntAddr::Var(_) => bail!("Getter returned a variable-length address"),
    }
}
