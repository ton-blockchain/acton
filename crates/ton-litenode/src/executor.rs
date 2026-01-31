use crate::types::{BocBytes, Lt};
use anyhow::Context;
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{EmulationResult, Executor, RunTransactionArgs};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily};
use tycho_types::models::{ShardAccount, Transaction};
use tycho_types::prelude::HashBytes;

#[derive(Clone, Debug)]
pub struct ExecContext {
    pub lt: Lt,
    pub gen_utime: u32,
    pub rand_seed: Option<[u8; 32]>,
}

#[derive(Clone, Debug)]
pub struct ExecResult {
    pub tx_boc: BocBytes,
    pub new_account_boc: Option<BocBytes>,
    pub out_msgs_boc: Vec<BocBytes>,
    pub exit_code: i32,
}

pub trait TvmExecutor {
    fn execute(
        &self,
        old_account: Option<&BocBytes>,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
    ) -> anyhow::Result<ExecResult>;
}

pub struct TvmEmulatorAdapter {
    inner: Executor,
}

impl TvmEmulatorAdapter {
    pub fn new() -> anyhow::Result<Self> {
        let inner = Executor::new(ExecutorVerbosity::Short, None)?;
        Ok(Self { inner })
    }
}

impl TvmExecutor for TvmEmulatorAdapter {
    fn execute(
        &self,
        old_account: Option<&BocBytes>,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
    ) -> anyhow::Result<ExecResult> {
        use base64::Engine;
        // 1. Prepare inputs
        // Set config
        let config_b64 = base64::engine::general_purpose::STANDARD.encode(config);
        self.inner
            .set_config(&config_b64)
            .context("Failed to set config")?;

        let in_msg_b64 = base64::engine::general_purpose::STANDARD.encode(in_msg);

        // Prepare shard account
        let shard_account_b64 = if let Some(acc_bytes) = old_account {
            base64::engine::general_purpose::STANDARD.encode(acc_bytes)
        } else {
            // Create empty shard account
            let sa = ShardAccount {
                account: tycho_types::cell::Lazy::new(&tycho_types::models::OptionalAccount(None))?,
                last_trans_hash: HashBytes([0u8; 32]),
                last_trans_lt: 0,
            };
            let mut builder = CellBuilder::new();
            use tycho_types::cell::Store;
            sa.store_into(&mut builder, Cell::empty_context())?;
            let cell = builder.build()?;
            Boc::encode_base64(&cell)
        };

        let args = RunTransactionArgs {
            shard_account: shard_account_b64,
            now: ctx.gen_utime,
            lt: ctx.lt,
            random_seed: ctx.rand_seed,
            debug_enabled: false,
            ..Default::default()
        };

        // 2. Run
        let (res, _logs) = self
            .inner
            .run_transaction(&in_msg_b64, &args)
            .context("Emulator run failed")?;

        // 3. Process output
        match res {
            EmulationResult::Success(s) => {
                let tx_boc = base64::engine::general_purpose::STANDARD.decode(&s.transaction)?;
                let new_account_boc =
                    Some(base64::engine::general_purpose::STANDARD.decode(&s.shard_account)?);

                // Parse transaction to get out messages
                let tx_cell = Boc::decode_base64(&s.transaction)?;
                let tx = tx_cell.parse::<Transaction>()?;

                let out_msgs_boc = tx
                    .iter_out_msgs()
                    .filter_map(Result::ok)
                    .map(|msg| {
                        let mut builder = CellBuilder::new();
                        use tycho_types::cell::Store;
                        msg.store_into(&mut builder, Cell::empty_context())?;
                        let cell = builder.build()?;
                        Ok(Boc::encode(cell))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;

                Ok(ExecResult {
                    tx_boc,
                    new_account_boc,
                    out_msgs_boc,
                    exit_code: 0, // TODO: extract from tx if needed
                })
            }
            EmulationResult::Error(e) => {
                anyhow::bail!(
                    "TVM Execution Error: {} (exit: {:?})",
                    e.error,
                    e.vm_exit_code
                )
            }
        }
    }
}
