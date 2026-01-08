use retrace::trace::{Trace, TraceStep};
use retrace::{Network, retrace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    pub hash: String,
    pub status: String,
    pub account: String,
    pub sender: String,
    pub lt: u64,
    pub exit_code: i32,
    pub vm_steps: u32,
    pub gas_used_total: u64,
}

pub struct DebuggerState {
    pub current_step: usize,
    pub trace: Option<Trace>,
    pub transaction: Option<TransactionDetails>,
}

impl DebuggerState {
    pub fn new() -> Self {
        Self {
            current_step: 0,
            trace: None,
            transaction: None,
        }
    }

    pub async fn init_from_hash(&mut self, network: Network, hash: &str) -> anyhow::Result<()> {
        let result = retrace(network, hash, Default::default()).await?;

        let trace = Trace::new(&result.emulated_tx.vm_logs, None);

        let tx_details = TransactionDetails {
            hash: hash.to_string(),
            status: if result.state_update_hash_ok {
                "Success (Deterministic)"
            } else {
                "Success (Non-deterministic)"
            }
            .to_string(),
            account: result.in_msg.contract.to_string(),
            sender: result
                .in_msg
                .sender
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "External".to_string()),
            lt: result.emulated_tx.lt,
            exit_code: match result.emulated_tx.compute_info {
                retrace::ComputeInfo::Success { exit_code, .. } => exit_code,
                retrace::ComputeInfo::Skipped => -1,
            },
            vm_steps: match result.emulated_tx.compute_info {
                retrace::ComputeInfo::Success { vm_steps, .. } => vm_steps,
                retrace::ComputeInfo::Skipped => 0,
            },
            gas_used_total: result.money.total_fees, // Or use emulated_tx.compute_info.gas_used
        };

        self.trace = Some(trace);
        self.transaction = Some(tx_details);
        self.current_step = 0;

        Ok(())
    }

    pub fn reset(&mut self) {
        self.current_step = 0;
    }

    pub fn step(&mut self, delta: i32) -> Option<&TraceStep> {
        let trace = self.trace.as_ref()?;
        let new_step = (self.current_step as i32 + delta)
            .max(0)
            .min(trace.steps.len() as i32 - 1);
        self.current_step = new_step as usize;
        Some(&trace.steps[self.current_step])
    }

    pub fn get_current_step(&self) -> Option<&TraceStep> {
        self.trace.as_ref()?.steps.get(self.current_step)
    }

    pub fn get_transaction_details(&self) -> Option<&TransactionDetails> {
        self.transaction.as_ref()
    }

    pub fn total_steps(&self) -> usize {
        self.trace.as_ref().map(|t| t.steps.len()).unwrap_or(0)
    }

    pub fn search_opcode(&mut self, name: &str, forward: bool) -> Option<&TraceStep> {
        let trace = self.trace.as_ref()?;
        let range: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new((self.current_step + 1)..trace.steps.len())
        } else {
            Box::new((0..self.current_step).rev())
        };

        for i in range {
            if let TraceStep::Execute { instr, .. } = &trace.steps[i]
                && instr.to_uppercase().contains(&name.to_uppercase())
            {
                self.current_step = i;
                return Some(&trace.steps[i]);
            }
        }
        None
    }
}
