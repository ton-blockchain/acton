use crate::types::{BocBytes, Lt};
use anyhow::Context;
use std::sync::Arc;
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{EmulationResult, Executor, PrevBlocksInfo, RunTransactionArgs};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::models::{ComputePhase, Transaction, TxInfo};

#[derive(Clone, Debug)]
pub struct ExecContext {
    pub lt: Lt,
    pub gen_utime: u32,
    pub rand_seed: Option<[u8; 32]>,
    pub ignore_chksig: bool,
    pub prev_blocks_info: PrevBlocksInfo,
}

#[derive(Clone, Debug)]
pub struct ExecResult {
    pub tx: Transaction,
    pub tx_boc: BocBytes,
    pub new_account_boc: BocBytes,
    pub out_msg_cells: Vec<Cell>,
    pub vm_log: Arc<str>,
    pub executor_logs: Arc<str>,
    pub actions: Option<Arc<str>>,
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
        let inner = Executor::new(localnet_executor_verbosity(), None)?;
        Ok(Self { inner })
    }
}

pub(crate) fn localnet_executor_verbosity() -> ExecutorVerbosity {
    if std::env::var("ACTON_NODE_COVERAGE").is_ok_and(|value| value.trim() == "1") {
        ExecutorVerbosity::FullLocationStack
    } else {
        ExecutorVerbosity::Short
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
        let config_b64 = config.to_base64();
        self.inner
            .set_config(&config_b64)
            .context("Failed to set config")?;

        let in_msg_b64 = in_msg.to_base64();
        let shard_account_b64 = shard_account.to_base64();

        let args = RunTransactionArgs {
            libs: libs.map(BocBytes::to_base64),
            shard_account: shard_account_b64,
            now: ctx.gen_utime,
            lt: ctx.lt,
            random_seed: ctx.rand_seed,
            ignore_chksig: ctx.ignore_chksig,
            debug_enabled: false,
            prev_blocks_info: Some(ctx.prev_blocks_info.clone()),
            ..Default::default()
        };

        // 2. Run
        let (res, logs) = self
            .inner
            .run_transaction(&in_msg_b64, &args)
            .context("Emulator run failed")?;

        // 3. Process output
        match res {
            EmulationResult::Success(s) => {
                let tx_boc = BocBytes::from_base64(s.transaction.as_ref())?;
                let new_account_boc = BocBytes::from_base64(s.shard_account.as_ref())?;

                let tx_cell = Boc::decode(&tx_boc)?;
                let tx = tx_cell.parse::<Transaction>()?;

                let out_msg_cells = tx
                    .iter_out_msgs()
                    .filter_map(Result::ok)
                    .map(CellBuilder::build_from)
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(ExecResult {
                    actions: s.actions,
                    executor_logs: logs,
                    tx,
                    tx_boc,
                    new_account_boc,
                    out_msg_cells,
                    vm_log: s.vm_log,
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
