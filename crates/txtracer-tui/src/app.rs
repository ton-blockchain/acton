use ratatui::widgets::ListState;
use retrace::Network;
use retrace::trace::{Trace, TraceStep};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

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

pub struct App {
    pub current_step: usize,
    pub details_expanded: bool,
    pub show_docs: bool,
    pub trace: Option<Trace>,
    pub transaction: Option<TransactionDetails>,
    pub step_list_state: RefCell<ListState>,
}

impl App {
    pub fn new() -> App {
        App {
            current_step: 0,
            details_expanded: false,
            show_docs: false,
            trace: None,
            transaction: None,
            step_list_state: RefCell::new(ListState::default()),
        }
    }

    pub async fn init_from_hash(&mut self, network: Network, hash: &str) -> anyhow::Result<()> {
        let result = retrace::retrace(network, hash, Default::default()).await?;

        let trace = Trace::new(&result.emulated_tx.vm_logs, Some(375_000));

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
            gas_used_total: result.money.total_fees,
        };

        self.trace = Some(trace);
        self.transaction = Some(tx_details);
        self.current_step = 0;
        self.update_selection();

        Ok(())
    }

    fn update_selection(&self) {
        self.step_list_state
            .borrow_mut()
            .select(Some(self.current_step));
    }

    pub fn on_left(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            self.update_selection();
        }
    }

    pub fn on_right(&mut self) {
        if let Some(trace) = &self.trace
            && self.current_step < trace.steps.len() - 1
        {
            self.current_step += 1;
            self.update_selection();
        }
    }

    pub fn on_home(&mut self) {
        self.current_step = 0;
        self.update_selection();
    }

    pub fn on_end(&mut self) {
        if let Some(trace) = &self.trace
            && !trace.steps.is_empty()
        {
            self.current_step = trace.steps.len() - 1;
            self.update_selection();
        }
    }

    pub fn toggle_details(&mut self) {
        self.details_expanded = !self.details_expanded;
    }

    pub fn on_up(&mut self) {
        self.on_left();
    }

    pub fn on_down(&mut self) {
        self.on_right();
    }

    pub fn total_steps(&self) -> usize {
        self.trace.as_ref().map(|t| t.steps.len()).unwrap_or(0)
    }

    pub fn get_current_step(&self) -> Option<&TraceStep> {
        self.trace.as_ref()?.steps.get(self.current_step)
    }

    pub fn get_cumulative_gas(&self, step_idx: usize) -> usize {
        if let Some(trace) = &self.trace {
            trace
                .steps
                .iter()
                .take(step_idx + 1)
                .filter_map(|s| {
                    if let TraceStep::Execute { gas, .. } = s {
                        Some(*gas)
                    } else {
                        None
                    }
                })
                .sum()
        } else {
            0
        }
    }
}
