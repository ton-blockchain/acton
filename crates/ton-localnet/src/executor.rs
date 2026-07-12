use crate::types::{BocBytes, Lt};
use anyhow::Context;
use std::cell::RefCell;
use ton_emulator::is_external_not_accepted_error;
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{
    EmulationResult, Executor, PrevBlocksInfo, RunTransactionArgs, RunTransactionResultSuccess,
};
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
    pub actions: Option<BocBytes>,
}

pub enum FeeEstimationExecution {
    Executed(ExecResult),
    ExternalNotAccepted,
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

    fn execute_for_fee_estimation(
        &self,
        shard_account: &BocBytes,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
        libs: Option<&BocBytes>,
    ) -> anyhow::Result<FeeEstimationExecution> {
        self.execute(shard_account, in_msg, ctx, config, libs)
            .map(FeeEstimationExecution::Executed)
    }
}

pub struct TvmEmulatorAdapter {
    inner: Executor,
    last_config: RefCell<Option<BocBytes>>,
}

impl TvmEmulatorAdapter {
    pub fn new() -> anyhow::Result<Self> {
        let inner = Executor::new(ExecutorVerbosity::Short, None)?;
        Ok(Self {
            inner,
            last_config: RefCell::new(None),
        })
    }

    fn run_emulation(
        &self,
        shard_account: &BocBytes,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
        libs: Option<&BocBytes>,
    ) -> anyhow::Result<EmulationResult> {
        {
            let mut last_config = self.last_config.borrow_mut();
            if last_config.as_ref() != Some(config) {
                self.inner
                    .set_config(&config.to_base64())
                    .context("Failed to set config")?;
                *last_config = Some(config.clone());
            }
        }

        let args = RunTransactionArgs {
            libs: libs.map(BocBytes::to_base64),
            shard_account: shard_account.to_base64(),
            now: ctx.gen_utime,
            lt: ctx.lt,
            random_seed: ctx.rand_seed,
            ignore_chksig: ctx.ignore_chksig,
            debug_enabled: false,
            prev_blocks_info: Some(ctx.prev_blocks_info.clone()),
            ..Default::default()
        };
        self.inner
            .run_transaction(&in_msg.to_base64(), &args)
            .map(|(result, _)| result)
            .context("Emulator run failed")
    }

    fn decode_success(result: RunTransactionResultSuccess) -> anyhow::Result<ExecResult> {
        let tx_boc = BocBytes::from_base64(result.transaction.as_ref())?;
        let new_account_boc = BocBytes::from_base64(result.shard_account.as_ref())?;
        let actions = result
            .actions
            .as_deref()
            .map(BocBytes::from_base64)
            .transpose()?;
        let tx_cell = Boc::decode(&tx_boc)?;
        let tx = tx_cell.parse::<Transaction>()?;
        let out_msg_cells = tx
            .iter_out_msgs()
            .filter_map(Result::ok)
            .map(CellBuilder::build_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ExecResult {
            tx,
            tx_boc,
            new_account_boc,
            out_msg_cells,
            actions,
        })
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
        match self.run_emulation(shard_account, in_msg, ctx, config, libs)? {
            EmulationResult::Success(result) => Self::decode_success(result),
            EmulationResult::Error(error) => anyhow::bail!(
                "TVM Execution Error: {} (exit: {:?})",
                error.error,
                error.vm_exit_code
            ),
        }
    }

    fn execute_for_fee_estimation(
        &self,
        shard_account: &BocBytes,
        in_msg: &BocBytes,
        ctx: &ExecContext,
        config: &BocBytes,
        libs: Option<&BocBytes>,
    ) -> anyhow::Result<FeeEstimationExecution> {
        match self.run_emulation(shard_account, in_msg, ctx, config, libs)? {
            EmulationResult::Success(result) => {
                Self::decode_success(result).map(FeeEstimationExecution::Executed)
            }
            EmulationResult::Error(error)
                if error.external_not_accepted || is_external_not_accepted_error(&error.error) =>
            {
                Ok(FeeEstimationExecution::ExternalNotAccepted)
            }
            EmulationResult::Error(error) => anyhow::bail!(
                "TVM Execution Error: {} (exit: {:?})",
                error.error,
                error.vm_exit_code
            ),
        }
    }
}
