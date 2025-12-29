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
//! *   [`InstalledActions`]: Actions (messages and reservations) that the contract
//!     *queued* during its execution.
//! *   [`ExecutedActions`]: Actions that the sandbox actually *processed* during
//!     the action phase.
//!
//! # Example
//!
//! ```ignore
//! use retrace::trace::Trace;
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
use std::fmt::{Display, Formatter};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::RelaxedMessage;
use vmlogs::executor_parser::{ExecutorLine, parse_executor_lines};
use vmlogs::parser::{CellLike, VmLine, VmStack, VmStackValue};

/// A single step or event in the TVM execution trace.
#[derive(Debug)]
pub enum TraceStep {
    /// Normal instruction execution.
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
    FinalC5 {
        /// Hex representation of the c5 cell.
        cell: String,
    },
}

/// An action that was "installed" (queued) by the contract during execution.
/// These are extracted from `SENDRAWMSG` and `RAWRESERVE` instructions.
#[derive(Debug)]
pub enum InstalledAction {
    /// A message was queued for sending via `SENDRAWMSG`.
    Message(InstalledSendMessageAction),
    /// A currency reservation was made via `RAWRESERVE`.
    Reserve(InstalledReserveAction),
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
    pub fn message(&self) -> Option<RelaxedMessage<'_>> {
        self.msg_cell.parse::<RelaxedMessage>().ok()
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

/// An action that was actually executed by the sandbox during the action phase.
/// These are extracted from the executor logs.
#[derive(Debug, Clone)]
pub enum ExecutedAction {
    /// A message was successfully processed and sent.
    SendMessage {
        /// Hash of the message.
        hash: String,
        /// Account balance remaining after sending the message.
        remaining_balance: BigInt,
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
    },
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
    pub fn stack(&'_ self) -> Option<Vec<VmStackValue<'_>>> {
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
}

impl Display for Trace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for step in &self.steps {
            match step {
                TraceStep::Execute { instr, .. } => writeln!(f, "{}", instr)?,
                TraceStep::Exception {
                    errno,
                    message,
                    handled,
                } => {
                    if *handled {
                        writeln!(f, "Handled exception {}: {}", errno, message)?;
                    } else {
                        writeln!(f, "Unhandled exception {}: {}", errno, message)?;
                    }
                }
                TraceStep::FinalC5 { cell } => {
                    writeln!(f, "Final C5: C{{{}}}", cell)?;
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
    pub fn new(vm_logs: &str, start_gas: Option<usize>) -> Self {
        let lines = vmlogs::parser::parse_lines(vm_logs);
        Self::from_lines(lines, start_gas)
    }

    /// Creates a new [`Trace`] from pre-parsed [`VmLine`]s.
    ///
    /// This method performs the stateful reconstruction of the trace by
    /// tracking gas changes and instruction metadata across lines.
    ///
    /// # Arguments
    ///
    /// * `lines` — A vector of results, each containing a parsed [`VmLine`] or an error string.
    /// * `start_gas` — Optional initial gas limit.
    pub fn from_lines(lines: Vec<Result<VmLine, String>>, start_gas: Option<usize>) -> Trace {
        let start_gas = start_gas.unwrap_or(1_000_000);
        let mut gas_remaining = start_gas;

        let mut steps = Vec::<TraceStep>::new();
        let mut current_hash: Option<String> = None;
        let mut current_offset: Option<String> = None;
        let mut current_instr: Option<String> = None;
        let mut current_stack: Option<String> = None;

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
                VmLine::VmStack { stack } => {
                    current_stack = Some(stack.raw().to_owned());
                }
                VmLine::VmGasRemaining { gas } => {
                    let new_gas = gas.parse::<usize>().unwrap_or(gas_remaining);
                    let gas_cost = gas_remaining.saturating_sub(new_gas);
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
                            stack: "".to_owned(),
                            offset: 0,
                            hash: "".to_owned(),
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
                        CellLike::Cell(h) => h.to_owned(),
                        CellLike::Builder(h) => h.to_owned(),
                    };
                    steps.push(TraceStep::FinalC5 { cell });
                }
                VmLine::VmLimitChanged { limit } => {
                    gas_remaining = limit.parse().unwrap_or(gas_remaining);
                }
                _ => {}
            }
        }

        Trace { steps, start_gas }
    }

    /// Extracts all [`InstalledAction`]s from the execution trace.
    ///
    /// Scans the trace for instructions that modify the action list (c5),
    /// specifically `SENDRAWMSG` and `RAWRESERVE`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let actions = trace.actions();
    /// println!("Total queued actions: {}", actions.actions.len());
    /// ```
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
                    if instr == "SENDRAWMSG" {
                        let parsed = VmStack::new(stack).parsed();
                        if parsed.len() < 2 {
                            return None;
                        }
                        // SENDRAWMSG takes (cell, mode) from the stack.
                        // We are interested in the cell (second from top).
                        if let Some(VmStackValue::Cell(CellLike::Cell(msg_cell))) =
                            parsed.get(parsed.len() - 2)
                            && let Ok(cell) = Boc::decode_hex(msg_cell)
                        {
                            return Some(InstalledAction::Message(InstalledSendMessageAction {
                                msg_hash: cell.repr_hash().to_string().to_ascii_uppercase(),
                                msg_cell: cell,
                                loc_hash: hash.clone(),
                                loc_offset: *offset,
                            }));
                        }
                    }

                    if instr == "RAWRESERVE" {
                        let parsed = VmStack::new(stack).parsed();
                        if parsed.len() < 2 {
                            return None;
                        }
                        // RAWRESERVE takes (amount, mode) from the stack.
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
                }
                None
            })
            .collect();
        InstalledActions { actions }
    }
}

/// A collection of actions that were queued (installed) during contract execution.
pub struct InstalledActions {
    /// The list of installed actions.
    pub actions: Vec<InstalledAction>,
}

impl InstalledActions {
    /// Creates an empty collection of installed actions.
    pub fn empty() -> Self {
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
    pub fn find_message(&self, hash: &String) -> Option<&InstalledSendMessageAction> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                InstalledAction::Message(msg) => Some(msg),
                InstalledAction::Reserve(_) => None,
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
    pub fn find_reserve(&self, mode: i32, amount: &BigInt) -> Option<&InstalledReserveAction> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                InstalledAction::Reserve(reserve) => Some(reserve),
                InstalledAction::Message(_) => None,
            })
            .find(|reserve| reserve.mode == mode && reserve.amount == *amount)
    }
}

/// A collection of actions that were actually executed by the sandbox.
pub struct ExecutedActions {
    /// The list of executed actions.
    pub actions: Vec<ExecutedAction>,
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
    pub fn from(logs: &str) -> ExecutedActions {
        let parsed_lines = parse_executor_lines(logs);
        let mut actions = Vec::new();

        for result in parsed_lines {
            match result {
                Ok(ExecutorLine::ProcessSendMessage { message_hash }) => {
                    actions.push(ExecutedAction::SendMessage {
                        hash: message_hash.to_string(),
                        remaining_balance: BigInt::from(0), // Will be updated by RemainingBalance
                    });
                }
                Ok(ExecutorLine::RemainingBalance { balance }) => {
                    if let Some(ExecutedAction::SendMessage {
                        remaining_balance, ..
                    }) = actions.last_mut()
                    {
                        *remaining_balance = balance.parse::<BigInt>().unwrap_or(BigInt::from(0));
                    }
                }
                Ok(ExecutorLine::ProcessRawReserve { mode }) => {
                    actions.push(ExecutedAction::ReserveCurrency {
                        mode: mode.parse().unwrap_or(0),
                        reserve: BigInt::from(0), // Will be filled from ActionReserveCurrency if available
                        balance: BigInt::from(0),
                        original_balance: BigInt::from(0),
                        changed_remaining_balance: BigInt::from(0), // Will be updated by ChangedBalance
                        changed_reserved_balance: BigInt::from(0), // Will be updated by ChangedBalance
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
                        *r = reserve.parse().unwrap_or(BigInt::from(0));
                        *b = balance.parse().unwrap_or(BigInt::from(0));
                        *ob = original_balance.parse().unwrap_or(BigInt::from(0));
                    } else {
                        actions.push(ExecutedAction::ReserveCurrency {
                            mode: mode_val,
                            reserve: reserve.parse().unwrap_or(BigInt::from(0)),
                            balance: balance.parse().unwrap_or(BigInt::from(0)),
                            original_balance: original_balance.parse().unwrap_or(BigInt::from(0)),
                            changed_remaining_balance: BigInt::from(0), // Will be updated by ChangedBalance
                            changed_reserved_balance: BigInt::from(0), // Will be updated by ChangedBalance
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
                        *changed_remaining_balance = remaining_balance
                            .parse::<BigInt>()
                            .unwrap_or(BigInt::from(0));
                        *changed_reserved_balance = reserved_balance
                            .parse::<BigInt>()
                            .unwrap_or(BigInt::from(0));
                    }
                }
                _ => {}
            }
        }

        ExecutedActions { actions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace() {
        let logs = r#"
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
        "#;

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
        let logs = r#"
execute LDSTDADDR
handling exception code 9: cannot load a MsgAddressInt
default exception handler, terminating vm with exit code 9
final c5: C{B5EE9C72010101010002000000}
        "#;
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
        let logs = r#"
execute CTOS
handling exception code 9: failed to load library cell
execute FOO
        "#;
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
}
