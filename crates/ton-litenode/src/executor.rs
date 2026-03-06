use crate::types::{BocBytes, Lt};
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{EmulationResult, Executor, RunTransactionArgs};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily};
use tycho_types::models::{ComputePhase, Transaction, TxInfo};

#[derive(Clone, Debug)]
pub struct ExecContext {
    pub lt: Lt,
    pub gen_utime: u32,
    pub rand_seed: Option<[u8; 32]>,
    pub ignore_chksig: bool,
}

#[derive(Clone, Debug)]
pub struct ExecResult {
    pub tx: Transaction,
    pub tx_boc: BocBytes,
    pub new_account_boc: Option<BocBytes>,
    pub out_msgs_boc: Vec<BocBytes>,
}

impl ExecResult {
    #[must_use]
    pub fn compute_exit_code(&self) -> Option<i32> {
        let info = self.tx.info.load().ok()?;
        let TxInfo::Ordinary(info) = info else {
            return None;
        };
        let ComputePhase::Executed(info) = info.compute_phase else {
            return None;
        };
        Some(info.exit_code)
    }

    #[must_use]
    pub fn action_result_code(&self) -> Option<i32> {
        let info = self.tx.info.load().ok()?;
        let TxInfo::Ordinary(info) = info else {
            return None;
        };
        let info = info.action_phase?;
        Some(info.result_code)
    }
}

pub trait TvmExecutor {
    fn execute(
        &self,
        shard_account: &BocBytes,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
        libs: Option<&BocBytes>,
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
        shard_account: &BocBytes,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
        libs: Option<&BocBytes>,
    ) -> anyhow::Result<ExecResult> {
        // 1. Prepare inputs
        let config_b64 = STANDARD.encode(config);
        self.inner
            .set_config(&config_b64)
            .context("Failed to set config")?;

        let in_msg_b64 = STANDARD.encode(in_msg);
        let shard_account_b64 = STANDARD.encode(shard_account);

        let args = RunTransactionArgs {
            libs: libs.map(|value| STANDARD.encode(value)),
            shard_account: shard_account_b64,
            now: ctx.gen_utime,
            lt: ctx.lt,
            random_seed: ctx.rand_seed,
            ignore_chksig: ctx.ignore_chksig,
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
                let tx_boc = BocBytes::from(STANDARD.decode(s.transaction.as_ref())?);
                let new_account_boc =
                    Some(BocBytes::from(STANDARD.decode(s.shard_account.as_ref())?));

                let tx_cell = Boc::decode_base64(s.transaction.as_ref())?;
                let tx = tx_cell.parse::<Transaction>()?;

                let out_msgs_boc = tx
                    .iter_out_msgs()
                    .filter_map(Result::ok)
                    .map(|msg| {
                        let mut builder = CellBuilder::new();
                        use tycho_types::cell::Store;
                        msg.store_into(&mut builder, Cell::empty_context())?;
                        let cell = builder.build()?;
                        Ok(BocBytes::from(Boc::encode(cell)))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;

                Ok(ExecResult {
                    tx,
                    tx_boc,
                    new_account_boc,
                    out_msgs_boc,
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
