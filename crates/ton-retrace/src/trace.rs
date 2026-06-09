//! Utilities for parsing and working with TVM execution traces.
//!
//! A [`Trace`] is a high-level representation of a TVM transaction's execution,
//! reconstructed from raw VM logs. It provides a structured view of every
//! instruction executed, gas consumed, stack states, and any exceptions or
//! action-list changes (c5) that occurred.
//!
//! # Main Components
//!
//! *   [`Trace`]: The primary container for an execution trace, consisting of a
//!     sequence of [`TraceStep`]s.
//! *   [`TraceStep`]: Individual events in the trace, such as instruction
//!     executions ([`TraceStep::Execute`]), exceptions ([`TraceStep::Exception`]),
//!     or action register value ([`TraceStep::FinalC5`]).
//! *   [`InstalledActions`]: Actions that the contract
//!     *queued* during its execution.
//! *   [`ExecutedActions`]: Actions that the sandbox actually *processed* during
//!     the action phase.
//!
//! # Example
//!
//! ```ignore
//! use ton_retrace::trace::Trace;
//!
//! // Create a trace from raw VM logs
//! let vm_logs = "..."; // raw logs from emulator
//! let trace = Trace::new(vm_logs, Some(1000000));
//!
//! // Iterate over steps
//! for step in &trace.steps {
//!     println!("{:?}", step);
//! }
//!
//! // Extract actions queued by the contract
//! let actions = trace.actions();
//! for action in actions.actions {
//!     println!("Queued action: {:?}", action);
//! }
//! ```

use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tvm_logs::executor_parser::{ExecutorLine, parse_executor_lines};
use tvm_logs::parser::{CellLike, VmLine, VmStack, VmStackValue};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::RelaxedMessage;

/// A single step or event in the TVM execution trace.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum TraceStep {
    /// Normal instruction execution.
    #[serde(rename = "execute")]
    Execute {
        /// The VM instruction being executed (e.g., `SETCP`, `DICTPUSHCONST`).
        instr: String,
        /// Raw string representation of the VM stack *before* the instruction.
        stack: String,
        /// Offset within the code cell.
        offset: u16,
        /// Hex hash of the code cell being executed.
        hash: String,
        /// Gas consumed by this specific instruction.
        gas: usize,
    },
    /// An exception occurred during execution.
    #[serde(rename = "exception")]
    Exception {
        /// Exception error number (e.g., "9" for cell underflow).
        errno: String,
        /// Human-readable error message.
        message: String,
        /// Whether the exception was handled by a user-defined handler.
        /// `true` if it was caught, `false` if it reached the default handler and terminated the VM.
        handled: bool,
    },
    /// Final state of the c5 control register (action list).
    #[serde(rename = "final_c5")]
    FinalC5 {
        /// Hex representation of the c5 cell.
        cell: String,
    },
}

/// An action that was "installed" (queued) by the contract during execution.
/// These are extracted from instructions that append to the action list.
#[derive(Debug)]
pub enum InstalledAction {
    /// A message was queued for sending via `SENDRAWMSG`.
    Message(InstalledSendMessageAction),
    /// A currency reservation was made via `RAWRESERVE`.
    Reserve(InstalledReserveAction),
    /// Contract code update was queued via `SETCODE`.
    SetCode(InstalledSetCodeAction),
    /// Contract library collection update was queued via `SETLIBCODE` or `CHANGELIB`.
    ChangeLibrary(InstalledChangeLibraryAction),
}

impl InstalledAction {
    /// Hash of the code cell where the instruction was executed.
    #[must_use]
    pub fn loc_hash(&self) -> &str {
        match self {
            Self::Message(action) => &action.loc_hash,
            Self::Reserve(action) => &action.loc_hash,
            Self::SetCode(action) => &action.loc_hash,
            Self::ChangeLibrary(action) => &action.loc_hash,
        }
    }

    /// Offset within the code cell.
    #[must_use]
    pub const fn loc_offset(&self) -> u16 {
        match self {
            Self::Message(action) => action.loc_offset,
            Self::Reserve(action) => action.loc_offset,
            Self::SetCode(action) => action.loc_offset,
            Self::ChangeLibrary(action) => action.loc_offset,
        }
    }

    /// Returns whether an executor-log action describes the same action-list item.
    #[must_use]
    pub fn matches_executed_action(&self, action: &ExecutedAction) -> bool {
        match (self, action) {
            (Self::Message(installed), ExecutedAction::SendMessage { hash, .. }) => {
                installed.msg_hash.eq_ignore_ascii_case(hash)
            }
            (Self::Reserve(installed), ExecutedAction::ReserveCurrency { mode, reserve, .. }) => {
                installed.mode == *mode && installed.amount == *reserve
            }
            (Self::SetCode(installed), ExecutedAction::SetCode { new_code_hash, .. }) => {
                cell_hash_matches(&installed.new_code, new_code_hash)
            }
            (
                Self::ChangeLibrary(installed),
                ExecutedAction::ChangeLibrary {
                    mode,
                    lib_hash,
                    lib_ref,
                    ..
                },
            ) => installed.mode == *mode && library_ref_matches(&installed.lib, lib_hash, lib_ref),
            _ => false,
        }
    }
}

/// Details of a message queued via `SENDRAWMSG`.
#[derive(Debug)]
pub struct InstalledSendMessageAction {
    /// SHA256 hash of the message.
    pub msg_hash: String,
    /// The message cell itself.
    pub msg_cell: Cell,
    /// Hash of the code cell where the instruction was executed.
    pub loc_hash: String,
    /// Offset within the code cell.
    pub loc_offset: u16,
}

impl InstalledSendMessageAction {
    /// Parses the message cell into a [`RelaxedMessage`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(msg) = action.message() {
    ///     println!("Destination: {}", msg.info.dest());
    /// }
    /// ```
    #[must_use]
    pub fn message(&self) -> Option<RelaxedMessage<'_>> {
        self.msg_cell.parse::<RelaxedMessage<'_>>().ok()
    }
}

/// Details of a currency reservation made via `RAWRESERVE`.
#[derive(Debug)]
pub struct InstalledReserveAction {
    /// Reservation mode.
    pub mode: i32,
    /// Amount of nanoton to reserve.
    pub amount: BigInt,
    /// Hash of the code cell where the instruction was executed.
    pub loc_hash: String,
    /// Offset within the code cell.
    pub loc_offset: u16,
}

/// Details of a code update queued via `SETCODE`.
#[derive(Debug)]
pub struct InstalledSetCodeAction {
    /// A cell with new code.
    pub new_code: Cell,
    /// Hash of the code cell where the instruction was executed.
    pub loc_hash: String,
    /// Offset within the code cell.
    pub loc_offset: u16,
}

/// Library reference queued by `SETLIBCODE` or `CHANGELIB`.
#[derive(Debug)]
pub enum InstalledLibraryRef {
    /// Hash of the root cell of the library code.
    Hash(BigInt),
    /// Library code itself.
    Cell(Cell),
}

/// Details of a library collection update queued via `SETLIBCODE` or `CHANGELIB`.
#[derive(Debug)]
pub struct InstalledChangeLibraryAction {
    /// Library change mode.
    pub mode: i32,
    /// Library reference.
    pub lib: InstalledLibraryRef,
    /// Hash of the code cell where the instruction was executed.
    pub loc_hash: String,
    /// Offset within the code cell.
    pub loc_offset: u16,
}

/// An action that was actually executed by the sandbox during the action phase.
/// These are extracted from the executor logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutedActionFailureReason {
    /// Send message failed because there were not enough funds to cover transfer + forwarding fees.
    NotEnoughToncoinToSend {
        /// Remaining account balance when processing the action.
        remaining_balance: BigInt,
        /// Required amount including forwarding fees.
        required: BigInt,
    },
    /// Reserve action failed because requested amount exceeds available balance.
    CannotReserveToncoin {
        /// Requested reserve amount.
        requested: BigInt,
        /// Available balance at reservation time.
        available: BigInt,
    },
}

#[derive(Debug, Clone)]
pub enum ExecutedAction {
    /// A message was successfully processed and sent.
    SendMessage {
        /// Hash of the message.
        hash: String,
        /// Account balance remaining after sending the message.
        remaining_balance: BigInt,
        /// Optional detailed reason when this action failed.
        failure_reason: Option<ExecutedActionFailureReason>,
        /// Optional action-phase error code for this action.
        failure_code: Option<i32>,
    },
    /// A currency reservation was successfully processed.
    ReserveCurrency {
        /// Reservation mode.
        mode: i32,
        /// Amount reserved.
        reserve: BigInt,
        /// Current balance.
        balance: BigInt,
        /// Original balance before reservation.
        original_balance: BigInt,
        /// Balance remaining after reservation.
        changed_remaining_balance: BigInt,
        /// Total amount reserved so far.
        changed_reserved_balance: BigInt,
        /// Optional detailed reason when this action failed.
        failure_reason: Option<ExecutedActionFailureReason>,
        /// Optional action-phase error code for this action.
        failure_code: Option<i32>,
    },
    /// Contract code update was successfully processed.
    SetCode {
        /// Hash of the new code cell.
        new_code_hash: String,
        /// Optional action-phase error code for this action.
        failure_code: Option<i32>,
    },
    /// Contract library collection update was successfully processed.
    ChangeLibrary {
        /// Library change mode before executor normalization.
        mode: i32,
        /// Hash of the target library code.
        lib_hash: String,
        /// Whether executor saw the library reference as `cell` or `hash`.
        lib_ref: String,
        /// Optional action-phase error code for this action.
        failure_code: Option<i32>,
    },
}

/// Details of an `invalid action` entry from executor logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAction {
    /// Index of the failing action in action-list processing order.
    pub action_index: usize,
    /// Action-phase error code for this invalid action.
    pub error_code: i32,
    /// Whether this happened during action-list preprocessing.
    pub during_preprocessing: bool,
}

impl ExecutedAction {
    fn set_failure_reason(&mut self, reason: ExecutedActionFailureReason) {
        match self {
            ExecutedAction::SendMessage { failure_reason, .. }
            | ExecutedAction::ReserveCurrency { failure_reason, .. } => {
                *failure_reason = Some(reason);
            }
            ExecutedAction::SetCode { .. } | ExecutedAction::ChangeLibrary { .. } => {}
        }
    }

    const fn set_failure_code(&mut self, code: i32) {
        match self {
            ExecutedAction::SendMessage { failure_code, .. }
            | ExecutedAction::ReserveCurrency { failure_code, .. }
            | ExecutedAction::SetCode { failure_code, .. }
            | ExecutedAction::ChangeLibrary { failure_code, .. } => {
                *failure_code = Some(code);
            }
        }
    }
}

impl TraceStep {
    /// Parses the raw stack string into a list of [`VmStackValue`]s.
    /// Returns `None` if the step is not an `Execute` step.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(stack) = step.stack() {
    ///     for value in stack {
    ///         println!("Stack value: {:?}", value);
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn stack(&'_ self) -> Option<Vec<VmStackValue>> {
        match self {
            TraceStep::Execute { stack, .. } => Some(VmStack::new(stack).parsed()),
            _ => None,
        }
    }
}

/// A full TVM execution trace.
#[derive(Debug)]
pub struct Trace {
    /// The initial gas limit at the start of execution.
    /// Used for gas consumption calculation.
    pub start_gas: usize,
    /// Sequential list of all execution steps.
    pub steps: Vec<TraceStep>,
    /// Full `BoC` hex payloads registered by compact VM stack logs, keyed by cell hash.
    pub registered_cell_bocs: HashMap<String, String>,
}

impl Display for Trace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for step in &self.steps {
            match step {
                TraceStep::Execute { instr, .. } => writeln!(f, "{instr}")?,
                TraceStep::Exception {
                    errno,
                    message,
                    handled,
                } => {
                    if *handled {
                        writeln!(f, "Handled exception {errno}: {message}")?;
                    } else {
                        writeln!(f, "Unhandled exception {errno}: {message}")?;
                    }
                }
                TraceStep::FinalC5 { cell } => {
                    writeln!(f, "Final C5: C{{{cell}}}")?;
                }
            }
        }

        Ok(())
    }
}

impl Trace {
    /// Creates a new [`Trace`] from raw VM logs.
    ///
    /// # Arguments
    ///
    /// * `vm_logs` — Raw string containing VM execution logs.
    /// * `start_gas` — Optional initial gas limit. Defaults to 1,000,000 if not provided.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let trace = Trace::new(vm_logs, Some(5000));
    /// ```
    #[must_use]
    pub fn new(vm_logs: &str, start_gas: Option<usize>) -> Self {
        Self::from_lines(tvm_logs::parser::parse_lines(vm_logs), start_gas)
    }

    /// Creates a new [`Trace`] from pre-parsed [`VmLine`]s.
    ///
    /// This method performs the stateful reconstruction of the trace by
    /// tracking gas changes and instruction metadata across lines.
    ///
    /// # Arguments
    ///
    /// * `lines` — Parsed VM lines or parsing errors.
    /// * `start_gas` — Optional initial gas limit.
    #[must_use]
    pub fn from_lines<'a>(
        lines: impl IntoIterator<Item = Result<VmLine<'a>, String>>,
        start_gas: Option<usize>,
    ) -> Trace {
        let start_gas = start_gas.unwrap_or(1_000_000);
        let mut gas_base = start_gas;
        let mut gas_consumed = 0usize;
        let mut gas_remaining = start_gas;

        let mut steps = Vec::<TraceStep>::new();
        let mut current_hash: Option<String> = None;
        let mut current_offset: Option<String> = None;
        let mut current_instr: Option<String> = None;
        let mut current_stack: Option<String> = None;
        let mut registered_cell_bocs = HashMap::new();

        for line_result in lines {
            let Ok(line) = line_result else { continue };

            match line {
                VmLine::VmLoc { hash, offset } => {
                    current_hash = Some(hash.to_owned());
                    current_offset = Some(offset.to_owned());
                }
                VmLine::VmExecute { instr } => {
                    current_instr = Some(instr.to_owned());
                }
                VmLine::VmRegisteredCell { hash, boc } => {
                    registered_cell_bocs.insert(hash.to_owned(), boc.to_owned());
                }
                VmLine::VmStack { stack } => {
                    current_stack = Some(stack.raw().to_owned());
                }
                VmLine::VmGasRemaining { gas } => {
                    let new_gas = gas.parse::<usize>().unwrap_or(gas_remaining);
                    let new_gas_consumed = gas_base.saturating_sub(new_gas);
                    let gas_cost = new_gas_consumed.saturating_sub(gas_consumed);
                    gas_consumed = new_gas_consumed;
                    gas_remaining = new_gas;

                    let instr = current_instr.take().unwrap_or_default();

                    if let (Some(hash), Some(offset_str), Some(stack)) = (
                        current_hash.take(),
                        current_offset.take(),
                        current_stack.take(),
                    ) {
                        let offset = offset_str.parse().unwrap_or(0);
                        steps.push(TraceStep::Execute {
                            instr,
                            stack,
                            offset,
                            hash,
                            gas: gas_cost,
                        });
                    } else {
                        // very unlikely
                        steps.push(TraceStep::Execute {
                            instr,
                            stack: String::new(),
                            offset: 0,
                            hash: String::new(),
                            gas: gas_cost,
                        });
                    }
                }
                VmLine::VmException { errno, message } => {
                    steps.push(TraceStep::Exception {
                        errno: errno.to_owned(),
                        message: message.to_owned(),
                        handled: true,
                    });
                }
                VmLine::VmExceptionHandler { .. } => {
                    if let Some(TraceStep::Exception { handled, .. }) = steps.last_mut() {
                        *handled = false;
                    }
                }
                VmLine::VmFinalC5 { value } => {
                    let cell = match value {
                        CellLike::Builder(h) | CellLike::Cell(h) => h.clone(),
                    };
                    steps.push(TraceStep::FinalC5 { cell });
                }
                VmLine::VmLimitChanged { limit } => {
                    if let Ok(new_limit) = limit.parse::<usize>() {
                        gas_base = new_limit;
                        gas_remaining = gas_base.saturating_sub(gas_consumed);
                    }
                }
                VmLine::VmUnknown { .. } => {}
            }
        }

        Trace {
            start_gas,
            steps,
            registered_cell_bocs,
        }
    }

    /// Extracts all [`InstalledAction`]s from the execution trace.
    ///
    /// Scans the trace for instructions that modify the action list (c5).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let actions = trace.actions();
    /// println!("Total queued actions: {}", actions.actions.len());
    /// ```
    #[must_use]
    pub fn actions(&self) -> InstalledActions {
        let actions = self
            .steps
            .iter()
            .filter_map(|step| {
                if let TraceStep::Execute {
                    instr,
                    stack,
                    hash,
                    offset,
                    ..
                } = step
                {
                    let parsed = VmStack::new(stack).parsed();

                    match instr.as_str() {
                        // SENDRAWMSG takes (cell, mode) from the stack.
                        // We are interested in the cell (second from top).
                        "SENDRAWMSG" if parsed.len() >= 2 => {
                            if let Some(VmStackValue::Cell(cell_like)) =
                                parsed.get(parsed.len() - 2)
                                && let Some(cell) =
                                    decode_stack_cell(cell_like, &self.registered_cell_bocs)
                            {
                                return Some(InstalledAction::Message(
                                    InstalledSendMessageAction {
                                        msg_hash: cell.repr_hash().to_string().to_ascii_uppercase(),
                                        msg_cell: cell,
                                        loc_hash: hash.clone(),
                                        loc_offset: *offset,
                                    },
                                ));
                            }
                        }
                        // RAWRESERVE takes (amount, mode) from the stack.
                        "RAWRESERVE" if parsed.len() >= 2 => {
                            if let (
                                Some(VmStackValue::Integer(amount_str)),
                                Some(VmStackValue::Integer(mode_str)),
                            ) = (parsed.get(parsed.len() - 2), parsed.last())
                            {
                                let amount = amount_str.parse().unwrap_or_default();
                                let mode = mode_str.parse().unwrap_or(0);
                                return Some(InstalledAction::Reserve(InstalledReserveAction {
                                    mode,
                                    amount,
                                    loc_hash: hash.clone(),
                                    loc_offset: *offset,
                                }));
                            }
                        }
                        // SETCODE takes (code) from the stack.
                        "SETCODE" if !parsed.is_empty() => {
                            if let Some(VmStackValue::Cell(cell_like)) = parsed.last()
                                && let Some(new_code) =
                                    decode_stack_cell(cell_like, &self.registered_cell_bocs)
                            {
                                return Some(InstalledAction::SetCode(InstalledSetCodeAction {
                                    new_code,
                                    loc_hash: hash.clone(),
                                    loc_offset: *offset,
                                }));
                            }
                        }
                        // SETLIBCODE takes (code, mode) from the stack.
                        "SETLIBCODE" if parsed.len() >= 2 => {
                            if let (
                                Some(VmStackValue::Cell(cell_like)),
                                Some(VmStackValue::Integer(mode_str)),
                            ) = (parsed.get(parsed.len() - 2), parsed.last())
                                && let Some(cell) =
                                    decode_stack_cell(cell_like, &self.registered_cell_bocs)
                            {
                                let mode = mode_str.parse().unwrap_or(0);
                                return Some(InstalledAction::ChangeLibrary(
                                    InstalledChangeLibraryAction {
                                        mode,
                                        lib: InstalledLibraryRef::Cell(cell),
                                        loc_hash: hash.clone(),
                                        loc_offset: *offset,
                                    },
                                ));
                            }
                        }
                        // CHANGELIB takes (hash, mode) from the stack.
                        "CHANGELIB" if parsed.len() >= 2 => {
                            if let (
                                Some(VmStackValue::Integer(hash_str)),
                                Some(VmStackValue::Integer(mode_str)),
                            ) = (parsed.get(parsed.len() - 2), parsed.last())
                            {
                                let lib_hash = hash_str.parse().unwrap_or_default();
                                let mode = mode_str.parse().unwrap_or(0);
                                return Some(InstalledAction::ChangeLibrary(
                                    InstalledChangeLibraryAction {
                                        mode,
                                        lib: InstalledLibraryRef::Hash(lib_hash),
                                        loc_hash: hash.clone(),
                                        loc_offset: *offset,
                                    },
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                None
            })
            .collect();
        InstalledActions { actions }
    }
}

fn decode_stack_cell(
    cell_like: &CellLike,
    registered_cell_bocs: &HashMap<String, String>,
) -> Option<Cell> {
    match cell_like {
        CellLike::Cell(cell) => {
            let boc = registered_cell_bocs
                .get(cell.as_str())
                .map_or(cell.as_str(), String::as_str);
            Boc::decode_hex(boc).ok()
        }
        CellLike::Builder(_) => None,
    }
}

fn cell_hash_matches(cell: &Cell, hash: &str) -> bool {
    cell.repr_hash().to_string().eq_ignore_ascii_case(hash)
}

fn library_ref_matches(lib: &InstalledLibraryRef, hash: &str, ref_kind: &str) -> bool {
    match (lib, ref_kind) {
        (InstalledLibraryRef::Cell(cell), "cell") => cell_hash_matches(cell, hash),
        (InstalledLibraryRef::Hash(installed_hash), "hash") => {
            BigInt::parse_bytes(hash.as_bytes(), 16)
                .is_some_and(|executed_hash| installed_hash == &executed_hash)
        }
        _ => false,
    }
}

/// A collection of actions that were queued (installed) during contract execution.
pub struct InstalledActions {
    /// The list of installed actions.
    pub actions: Vec<InstalledAction>,
}

impl InstalledActions {
    /// Creates an empty collection of installed actions.
    #[must_use]
    pub const fn empty() -> Self {
        Self { actions: vec![] }
    }

    /// Finds a specific queued message by its hash.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(msg) = actions.find_message(&msg_hash) {
    ///     println!("Found queued message at offset {}", msg.loc_offset);
    /// }
    /// ```
    #[must_use]
    pub fn find_message(&self, hash: &String) -> Option<&InstalledSendMessageAction> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                InstalledAction::Message(msg) => Some(msg),
                _ => None,
            })
            .find(|msg| msg.msg_hash == *hash)
    }

    /// Finds a specific queued currency reservation by mode and amount.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(reserve) = actions.find_reserve(0, &BigInt::from(1000000)) {
    ///     println!("Found reservation at offset {}", reserve.loc_offset);
    /// }
    /// ```
    #[must_use]
    pub fn find_reserve(&self, mode: i32, amount: &BigInt) -> Option<&InstalledReserveAction> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                InstalledAction::Reserve(reserve) => Some(reserve),
                _ => None,
            })
            .find(|reserve| reserve.mode == mode && reserve.amount == *amount)
    }

    /// Finds an installed action by its action-list index.
    #[must_use]
    pub fn find_by_index(&self, index: usize) -> Option<&InstalledAction> {
        self.actions.get(index)
    }
}

/// A collection of actions that were actually executed by the sandbox.
pub struct ExecutedActions {
    /// The list of executed actions.
    pub actions: Vec<ExecutedAction>,
    /// Raw `invalid action ...` entries from executor logs.
    pub invalid_actions: Vec<InvalidAction>,
}

impl ExecutedActions {
    /// Extracts detailed information about executed actions from executor logs.
    ///
    /// These logs are produced during the sandbox's action phase, after the
    /// compute phase has finished.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let executed_actions = ExecutedActions::from(&result.emulated_tx.executor_logs);
    /// for action in executed_actions.actions {
    ///     println!("Executed: {:?}", action);
    /// }
    /// ```
    #[must_use]
    pub fn from(logs: &str) -> ExecutedActions {
        let parsed_lines = parse_executor_lines(logs);
        let mut actions = Vec::new();
        let mut invalid_actions = Vec::new();

        for result in parsed_lines {
            match result {
                Ok(ExecutorLine::ProcessSendMessage { message_hash }) => {
                    actions.push(ExecutedAction::SendMessage {
                        hash: message_hash.to_string(),
                        remaining_balance: BigInt::ZERO, // Will be updated by RemainingBalance
                        failure_reason: None,
                        failure_code: None,
                    });
                }
                Ok(ExecutorLine::ProcessSetCode { new_code_hash }) => {
                    actions.push(ExecutedAction::SetCode {
                        new_code_hash: new_code_hash.to_string(),
                        failure_code: None,
                    });
                }
                Ok(ExecutorLine::ProcessChangeLibrary {
                    mode,
                    lib_hash,
                    lib_ref,
                }) => {
                    actions.push(ExecutedAction::ChangeLibrary {
                        mode: mode.parse().unwrap_or(0),
                        lib_hash: lib_hash.to_string(),
                        lib_ref: lib_ref.to_string(),
                        failure_code: None,
                    });
                }
                Ok(ExecutorLine::RemainingBalance { balance }) => {
                    if let Some(ExecutedAction::SendMessage {
                        remaining_balance, ..
                    }) = actions.last_mut()
                    {
                        *remaining_balance = balance.parse::<BigInt>().unwrap_or(BigInt::ZERO);
                    }
                }
                Ok(ExecutorLine::ProcessRawReserve { mode }) => {
                    actions.push(ExecutedAction::ReserveCurrency {
                        mode: mode.parse().unwrap_or(0),
                        reserve: BigInt::ZERO, // Will be filled from ActionReserveCurrency if available
                        balance: BigInt::ZERO,
                        original_balance: BigInt::ZERO,
                        changed_remaining_balance: BigInt::ZERO, // Will be updated by ChangedBalance
                        changed_reserved_balance: BigInt::ZERO, // Will be updated by ChangedBalance
                        failure_reason: None,
                        failure_code: None,
                    });
                }
                Ok(ExecutorLine::ActionReserveCurrency {
                    mode,
                    reserve,
                    balance,
                    original_balance,
                }) => {
                    let mode_val = mode.parse().unwrap_or(0);
                    if let Some(ExecutedAction::ReserveCurrency {
                        reserve: r,
                        balance: b,
                        original_balance: ob,
                        ..
                    }) = actions.last_mut()
                    {
                        *r = reserve.parse().unwrap_or(BigInt::ZERO);
                        *b = balance.parse().unwrap_or(BigInt::ZERO);
                        *ob = original_balance.parse().unwrap_or(BigInt::ZERO);
                    } else {
                        actions.push(ExecutedAction::ReserveCurrency {
                            mode: mode_val,
                            reserve: reserve.parse().unwrap_or(BigInt::ZERO),
                            balance: balance.parse().unwrap_or(BigInt::ZERO),
                            original_balance: original_balance.parse().unwrap_or(BigInt::ZERO),
                            changed_remaining_balance: BigInt::ZERO, // Will be updated by ChangedBalance
                            changed_reserved_balance: BigInt::ZERO, // Will be updated by ChangedBalance
                            failure_reason: None,
                            failure_code: None,
                        });
                    }
                }
                Ok(ExecutorLine::ChangedBalance {
                    remaining_balance,
                    reserved_balance,
                }) => {
                    if let Some(ExecutedAction::ReserveCurrency {
                        changed_remaining_balance,
                        changed_reserved_balance,
                        ..
                    }) = actions.last_mut()
                    {
                        *changed_remaining_balance =
                            remaining_balance.parse::<BigInt>().unwrap_or(BigInt::ZERO);
                        *changed_reserved_balance =
                            reserved_balance.parse::<BigInt>().unwrap_or(BigInt::ZERO);
                    }
                }
                Ok(ExecutorLine::NotEnoughGramsToTransfer {
                    remaining_balance,
                    required,
                }) => {
                    if let Some(action) = actions.last_mut()
                        && matches!(action, ExecutedAction::SendMessage { .. })
                    {
                        action.set_failure_reason(
                            ExecutedActionFailureReason::NotEnoughToncoinToSend {
                                remaining_balance: remaining_balance
                                    .parse::<BigInt>()
                                    .unwrap_or(BigInt::ZERO),
                                required: required.parse::<BigInt>().unwrap_or(BigInt::ZERO),
                            },
                        );
                    }
                }
                Ok(ExecutorLine::CannotReserve {
                    requested,
                    available,
                }) => {
                    if let Some(action) = actions.last_mut()
                        && matches!(action, ExecutedAction::ReserveCurrency { .. })
                    {
                        action.set_failure_reason(
                            ExecutedActionFailureReason::CannotReserveToncoin {
                                requested: requested.parse::<BigInt>().unwrap_or(BigInt::ZERO),
                                available: available.parse::<BigInt>().unwrap_or(BigInt::ZERO),
                            },
                        );
                    }
                }
                Ok(ExecutorLine::InvalidAction {
                    action_index,
                    error_code,
                    during_preprocessing,
                }) => {
                    let action_index = action_index.parse::<usize>().ok();
                    let error_code = error_code.parse::<i32>().ok();
                    if let (Some(idx), Some(code)) = (action_index, error_code) {
                        invalid_actions.push(InvalidAction {
                            action_index: idx,
                            error_code: code,
                            during_preprocessing,
                        });

                        if let Some(action) = actions.get_mut(idx) {
                            action.set_failure_code(code);
                        } else if !during_preprocessing && let Some(action) = actions.last_mut() {
                            // Keep historical fallback for logs where action index
                            // does not map to reconstructed action vector.
                            action.set_failure_code(code);
                        }
                    }
                }
                _ => {}
            }
        }

        ExecutedActions {
            actions,
            invalid_actions,
        }
    }
}

#[cfg(test)]
mod local_tests {
    use super::*;

    #[test]
    fn actions_resolve_registered_cell_boc() {
        let boc = "B5EE9C72010101010002000000";
        let logs = format!(
            r"
register new cell 0F: {boc}
stack: [ C{{0F}} 0 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 0
execute SENDRAWMSG
gas remaining: 999
        "
        );

        let trace = Trace::new(&logs, None);
        let actions = trace.actions();
        assert_eq!(actions.actions.len(), 1);
        let InstalledAction::Message(action) = &actions.actions[0] else {
            panic!("Expected installed message action");
        };
        let expected_hash = Boc::decode_hex(boc)
            .expect("test boc should decode")
            .repr_hash()
            .to_string()
            .to_ascii_uppercase();
        assert_eq!(action.msg_hash, expected_hash);
    }

    #[test]
    fn actions_collect_code_and_library_updates() {
        let boc = "B5EE9C72010101010002000000";
        let logs = format!(
            r"
register new cell 0F: {boc}
stack: [ C{{0F}} ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 10
execute SETCODE
gas remaining: 999
stack: [ C{{0F}} 2 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 20
execute SETLIBCODE
gas remaining: 998
stack: [ 12345 1 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 30
execute CHANGELIB
gas remaining: 997
        "
        );

        let trace = Trace::new(&logs, None);
        let actions = trace.actions();
        assert_eq!(actions.actions.len(), 3);

        let InstalledAction::SetCode(action) = &actions.actions[0] else {
            panic!("Expected installed set-code action");
        };
        assert_eq!(action.loc_offset, 10);
        assert_eq!(
            action.new_code.repr_hash(),
            Boc::decode_hex(boc)
                .expect("test boc should decode")
                .repr_hash()
        );

        let InstalledAction::ChangeLibrary(action) = &actions.actions[1] else {
            panic!("Expected installed set-library action");
        };
        assert_eq!(action.mode, 2);
        assert!(matches!(action.lib, InstalledLibraryRef::Cell(_)));
        assert_eq!(action.loc_offset, 20);

        let InstalledAction::ChangeLibrary(action) = &actions.actions[2] else {
            panic!("Expected installed change-library action");
        };
        assert_eq!(action.mode, 1);
        assert!(
            matches!(&action.lib, InstalledLibraryRef::Hash(hash) if hash == &BigInt::from(12345))
        );
        assert_eq!(action.loc_offset, 30);
    }

    #[test]
    fn installed_actions_match_set_code_and_change_library_executor_logs() {
        let boc = "B5EE9C72010101010002000000";
        let logs = format!(
            r"
register new cell 0F: {boc}
stack: [ C{{0F}} ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 10
execute SETCODE
gas remaining: 999
stack: [ C{{0F}} 18 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 20
execute SETLIBCODE
gas remaining: 998
stack: [ 12345 1 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 30
execute CHANGELIB
gas remaining: 997
        "
        );

        let trace = Trace::new(&logs, None);
        let actions = trace.actions();
        let cell_hash = Boc::decode_hex(boc)
            .expect("test boc should decode")
            .repr_hash()
            .to_string();
        let executor_logs = format!(
            "[ 4][t 0][2026-03-03 13:38:24.650053][transaction.cpp:2269]\tprocess set code {cell_hash}
[ 4][t 0][2026-03-03 13:38:24.650054][transaction.cpp:2312]\tprocess change library with mode 18, lib_hash={cell_hash}, lib_ref=cell
[ 4][t 0][2026-03-03 13:38:24.650055][transaction.cpp:2312]\tprocess change library with mode 1, lib_hash=3039, lib_ref=hash"
        );
        let executed = ExecutedActions::from(&executor_logs);

        assert_eq!(executed.actions.len(), 3);
        assert!(actions.actions[0].matches_executed_action(&executed.actions[0]));
        assert!(actions.actions[1].matches_executed_action(&executed.actions[1]));
        assert!(actions.actions[2].matches_executed_action(&executed.actions[2]));

        let mismatched_executor_logs = format!(
            "[ 4][t 0][2026-03-03 13:38:24.650054][transaction.cpp:2312]\tprocess change library with mode 18, lib_hash={cell_hash}, lib_ref=hash"
        );
        let mismatched = ExecutedActions::from(&mismatched_executor_logs);
        assert!(!actions.actions[1].matches_executed_action(&mismatched.actions[0]));
    }

    #[test]
    fn gas_cost_preserves_consumed_gas_when_limit_changes() {
        let logs = r"
stack: [ ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 0
execute DROP
gas remaining: 90
stack: [ ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 1
execute ACCEPT
changing gas limit to 1000
gas remaining: 980
        ";

        let trace = Trace::new(logs, Some(100));
        let gas_costs = trace
            .steps
            .iter()
            .filter_map(|step| match step {
                TraceStep::Execute { gas, .. } => Some(*gas),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(gas_costs, vec![10, 10]);
    }
}

#[cfg(all(test, feature = "only_ci"))]
mod tests {
    use super::*;

    #[test]
    fn test_trace() {
        let logs = r"
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} CS{B5EE9C72010101010002000000} 0 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 0
execute SETCP 0
gas remaining: 4974
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} CS{B5EE9C72010101010002000000} 0 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 16
execute DICTPUSHCONST 19 (xC_,1)
gas remaining: 4940
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} CS{B5EE9C72010101010002000000} 0 C{B5EE9C7201010201004300027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED5401010000} 19 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 40
execute DICTIGETJMPZ
gas remaining: 4814
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} CS{B5EE9C72010101010002000000} ]
code cell hash: 32F41F1164EC59D6A206558EA1876655ED1CB186A793E2E53D05AD265F319507 offset: 8
execute DROP
gas remaining: 4796
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} ]
code cell hash: 32F41F1164EC59D6A206558EA1876655ED1CB186A793E2E53D05AD265F319507 offset: 16
execute INMSG_BOUNCED
gas remaining: 4770
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} 0 ]
code cell hash: 32F41F1164EC59D6A206558EA1876655ED1CB186A793E2E53D05AD265F319507 offset: 32
execute THROWIF 0
gas remaining: 4744
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} ]
code cell hash: 32F41F1164EC59D6A206558EA1876655ED1CB186A793E2E53D05AD265F319507 offset: 48
execute PUSH c4
gas remaining: 4718
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} C{B5EE9C7201010601007A000114FF00F4A413F4BCF2C80B010201620203009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015804050005BBE1780017B8AD0ED44D0D31F31D70B1F8} ]
code cell hash: 32F41F1164EC59D6A206558EA1876655ED1CB186A793E2E53D05AD265F319507 offset: 64
execute GASCONSUMED
gas remaining: 4692
stack: [ 50000607 50000607 C{B5EE9C7201020A010001280002B34801EFD2E8E5A9093E903E6734A920503635CB733D16202D807D5EA2909CBAF757E33FD2955F3F91525CD4E9514C71D49B7D2CF8703DD1110467F7FE496BCE33949F29D00BEBCB7C08029E2BD000004CBCF1113784D26D049F1901020114FF00F4A413F4BCF2C80B030114FF00F4A413F4BCF2C80B05027AD330F891F240ED44F80721830AF94130F8075003A17FF83B0280647FF837A08010FB02F892C8CF8508FA5270CF0B6EC98306FB0072FB0688FB0488ED54040400000201620607009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015808090005BBE1780017B8AD0ED44D0D31F31D70B1F8} C{B5EE9C7201010601007A000114FF00F4A413F4BCF2C80B010201620203009CD0F8919130E020D72C23F43B277C8E1831ED44D001D70B1F01D61FD70B1F58A001C8CECB1FC9ED54E0D72C21D3A97834318E1230ED44D0D61F30C8CECF9000000002C9ED54E0810FF601C700F2F402015804050005BBE1780017B8AD0ED44D0D31F31D70B1F8} 308 ]
        ";

        let trace = Trace::new(logs, Some(5000));

        assert_eq!(trace.steps.len(), 8);
        if let TraceStep::Execute { gas, hash, .. } = &trace.steps[0] {
            assert_eq!(*gas, 26);
            assert_eq!(
                hash,
                "734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D"
            );
        } else {
            panic!("Expected Execute step at index 0");
        }
    }

    #[test]
    fn test_trace_exception() {
        let logs = r"
execute LDSTDADDR
handling exception code 9: cannot load a MsgAddressInt
default exception handler, terminating vm with exit code 9
final c5: C{B5EE9C72010101010002000000}
        ";
        let trace = Trace::new(logs, None);
        assert_eq!(trace.steps.len(), 2);

        if let TraceStep::Exception {
            errno,
            message,
            handled,
        } = &trace.steps[0]
        {
            assert_eq!(errno, "9");
            assert_eq!(message, "cannot load a MsgAddressInt");
            assert!(!*handled);
        } else {
            panic!("Expected Exception step at index 0");
        }

        if let TraceStep::FinalC5 { cell } = &trace.steps[1] {
            assert_eq!(cell, "B5EE9C72010101010002000000");
        } else {
            panic!("Expected FinalC5 step at index 1");
        }
    }

    #[test]
    fn test_trace_exception_handled() {
        let logs = r"
execute CTOS
handling exception code 9: failed to load library cell
execute FOO
        ";
        let trace = Trace::new(logs, None);
        assert_eq!(trace.steps.len(), 1);

        if let TraceStep::Exception {
            errno,
            message,
            handled,
        } = &trace.steps[0]
        {
            assert_eq!(errno, "9");
            assert_eq!(message, "failed to load library cell");
            assert!(*handled);
        } else {
            panic!("Expected Exception step at index 0");
        }
    }

    #[test]
    fn test_executed_actions_send_error_parsing() {
        let logs = r"[ 4][t 0][2026-02-25 11:22:27.910181][transaction.cpp:2649]	process send message 6B4A9BAD9FCCCE4523A71307366AF36EC1C535F5D05EF2FF21E358903A399123
[ 3][t 0][2026-02-25 11:22:27.910192][transaction.cpp:3070]	remaining balance 997209600ng
[ 4][t 0][2026-02-25 11:22:27.910194][transaction.cpp:2649]	process send message 52B0D905B98FC395D52C1EF89AB4F9BBF869AF0B1445E18DA4691C1FD2ACC22F
[ 4][t 0][2026-02-25 11:22:27.910199][transaction.cpp:2926]	not enough grams to transfer with the message : remaining balance is 997209600ng, need 1000000400000 (including forwarding fees)
[ 4][t 0][2026-02-25 11:22:27.910201][transaction.cpp:2206]	invalid action 1 in action list: error code 37";

        let executed = ExecutedActions::from(logs);
        assert_eq!(executed.actions.len(), 2);
        assert_eq!(executed.invalid_actions.len(), 1);
        assert_eq!(
            executed.invalid_actions[0],
            InvalidAction {
                action_index: 1,
                error_code: 37,
                during_preprocessing: false,
            }
        );

        if let ExecutedAction::SendMessage {
            hash,
            remaining_balance,
            failure_reason,
            failure_code,
        } = &executed.actions[0]
        {
            assert_eq!(
                hash,
                "6B4A9BAD9FCCCE4523A71307366AF36EC1C535F5D05EF2FF21E358903A399123"
            );
            assert_eq!(remaining_balance, &BigInt::from(997_209_600u64));
            assert!(failure_reason.is_none());
            assert!(failure_code.is_none());
        } else {
            panic!("Expected first action to be SendMessage");
        }

        if let ExecutedAction::SendMessage {
            hash,
            remaining_balance,
            failure_reason,
            failure_code,
        } = &executed.actions[1]
        {
            assert_eq!(
                hash,
                "52B0D905B98FC395D52C1EF89AB4F9BBF869AF0B1445E18DA4691C1FD2ACC22F"
            );
            assert_eq!(remaining_balance, &BigInt::ZERO);
            assert_eq!(failure_code, &Some(37));
            assert_eq!(
                failure_reason.as_ref(),
                Some(&ExecutedActionFailureReason::NotEnoughToncoinToSend {
                    remaining_balance: BigInt::from(997_209_600u64),
                    required: BigInt::from(1_000_000_400_000u64),
                })
            );
        } else {
            panic!("Expected second action to be SendMessage");
        }
    }

    #[test]
    fn test_executed_actions_reserve_error_parsing() {
        let logs = r"[ 4][t 0][2026-02-25 11:24:46.612154][transaction.cpp:3089]	process raw reserve with mode 0
[ 4][t 0][2026-02-25 11:24:46.612156][transaction.cpp:3108]	action_reserve_currency: mode=0, reserve=10000000ng, balance=1098500000ng, original balance=999742800ng
[ 3][t 0][2026-02-25 11:24:46.612158][transaction.cpp:3168]	changed remaining balance to 1088500000ng, reserved balance to 10000000ng
[ 4][t 0][2026-02-25 11:24:46.612160][transaction.cpp:3089]	process raw reserve with mode 0
[ 4][t 0][2026-02-25 11:24:46.612161][transaction.cpp:3108]	action_reserve_currency: mode=0, reserve=1000000000000ng, balance=1088500000ng, original balance=999742800ng
[ 4][t 0][2026-02-25 11:24:46.612163][transaction.cpp:3143]	cannot reserve 1000000000000 nanograms : only 1088500000 available
[ 4][t 0][2026-02-25 11:24:46.612164][transaction.cpp:2206]	invalid action 1 in action list: error code 37";

        let executed = ExecutedActions::from(logs);
        assert_eq!(executed.actions.len(), 2);
        assert_eq!(executed.invalid_actions.len(), 1);
        assert_eq!(
            executed.invalid_actions[0],
            InvalidAction {
                action_index: 1,
                error_code: 37,
                during_preprocessing: false,
            }
        );

        if let ExecutedAction::ReserveCurrency {
            mode,
            reserve,
            balance,
            original_balance,
            changed_remaining_balance,
            changed_reserved_balance,
            failure_reason,
            failure_code,
        } = &executed.actions[0]
        {
            assert_eq!(*mode, 0);
            assert_eq!(reserve, &BigInt::from(10_000_000u64));
            assert_eq!(balance, &BigInt::from(1_098_500_000u64));
            assert_eq!(original_balance, &BigInt::from(999_742_800u64));
            assert_eq!(changed_remaining_balance, &BigInt::from(1_088_500_000u64));
            assert_eq!(changed_reserved_balance, &BigInt::from(10_000_000u64));
            assert!(failure_reason.is_none());
            assert!(failure_code.is_none());
        } else {
            panic!("Expected first action to be ReserveCurrency");
        }

        if let ExecutedAction::ReserveCurrency {
            mode,
            reserve,
            balance,
            original_balance,
            changed_remaining_balance,
            changed_reserved_balance,
            failure_reason,
            failure_code,
        } = &executed.actions[1]
        {
            assert_eq!(*mode, 0);
            assert_eq!(reserve, &BigInt::from(1_000_000_000_000u64));
            assert_eq!(balance, &BigInt::from(1_088_500_000u64));
            assert_eq!(original_balance, &BigInt::from(999_742_800u64));
            assert_eq!(changed_remaining_balance, &BigInt::ZERO);
            assert_eq!(changed_reserved_balance, &BigInt::ZERO);
            assert_eq!(failure_code, &Some(37));
            assert_eq!(
                failure_reason.as_ref(),
                Some(&ExecutedActionFailureReason::CannotReserveToncoin {
                    requested: BigInt::from(1_000_000_000_000u64),
                    available: BigInt::from(1_088_500_000u64),
                })
            );
        } else {
            panic!("Expected second action to be ReserveCurrency");
        }
    }

    #[test]
    fn test_executed_actions_preprocessing_invalid_action_is_preserved() {
        let logs = r"[ 4][t 0][2026-03-03 13:38:24.650053][transaction.cpp:2160]	invalid action 0 found while preprocessing action list: error code 34";

        let executed = ExecutedActions::from(logs);
        assert!(executed.actions.is_empty());
        assert_eq!(
            executed.invalid_actions,
            vec![InvalidAction {
                action_index: 0,
                error_code: 34,
                during_preprocessing: true,
            }]
        );
    }
}
