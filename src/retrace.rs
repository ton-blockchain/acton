use crate::vmtrace;
use num_bigint::BigInt;
use num_traits::Zero;
use tolkc::source_map::{DebugLocation, SourceLocation, SourceMap};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::RelaxedMessage;
use vmlogs::executor_parser::{ExecutorLine, parse_executor_lines};
use vmlogs::parser::{CellLike, VmLine, VmStackValue};

#[derive(Debug)]
pub struct ExceptionInfo {
    pub description: String,
    pub loc: Option<SourceLocation>,
    pub backtrace: Vec<DebugLocation>,
}

pub fn find_exception_info(vm_logs: &String, source_map: &SourceMap) -> Option<ExceptionInfo> {
    let lines = vmlogs::parser::parse_lines(vm_logs.as_str());

    let exception = lines.iter().rfind(|line| match line {
        Ok(VmLine::VmException { .. }) => true,
        _ => false,
    });
    let description = match exception {
        Some(Ok(VmLine::VmException { message, .. })) => message.to_string(),
        _ => "".to_string(),
    };

    let location = lines.iter().rfind(|line| match line {
        Ok(VmLine::VmLoc { .. }) => true,
        _ => false,
    });

    let (hash, offset) = match location {
        Some(Ok(VmLine::VmLoc { hash, offset })) => (hash.to_string(), offset.parse().unwrap_or(0)),
        _ => ("".to_string(), 0),
    };

    let loc = find_source_loc(source_map, &hash, offset);

    let backtrace = find_backtrace(source_map, lines);

    Some(ExceptionInfo {
        description,
        loc,
        backtrace,
    })
}

fn find_backtrace(
    source_map: &SourceMap,
    lines: Vec<Result<VmLine, String>>,
) -> Vec<DebugLocation> {
    let execution_path = vmtrace::build_vm_trace_from_lines(lines, source_map);

    let mut stack = vec![];

    for step in &execution_path {
        if step.context.event == Some("EnterFunction".to_string())
            || step.context.event == Some("EnterInlinedFunction".to_string())
        {
            if step.context.event_function.is_none() {
                continue;
            }

            stack.push(step);
        }
        if step.context.event == Some("AfterFunctionCall".to_string())
            || step.context.event == Some("LeaveInlinedFunction".to_string())
        {
            let event_function = &step.context.event_function;

            let Some(last) = stack.last() else {
                continue;
            };

            if last.context.event_function == *event_function {
                stack.pop();
            }
        }
    }
    stack.iter().map(|loc| (**loc).clone()).collect::<Vec<_>>()
}

pub fn find_source_loc(
    source_map: &SourceMap,
    hash: &String,
    offset: i32,
) -> Option<SourceLocation> {
    if source_map.high_level.locations.is_empty() {
        // `--backtrace full` is not enabled
        return None;
    }

    let locs =
        vmtrace::low_level_loc_to_debug_locations(source_map, hash.as_str(), offset, false, true)?;
    locs.last().and_then(|l| Some(l.loc.clone()))
}

pub struct InstalledActions {
    pub actions: Vec<InstalledAction>,
}

impl InstalledActions {
    pub fn empty() -> Self {
        Self { actions: vec![] }
    }

    pub fn find_message(&self, hash: &String) -> Option<&InstalledSendMessageAction> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                InstalledAction::Message(msg) => Some(msg),
                InstalledAction::Reserve(_) => None,
            })
            .find(|msg| msg.msg_hash == *hash)
    }

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

pub enum InstalledAction {
    Message(InstalledSendMessageAction),
    Reserve(InstalledReserveAction),
}

pub struct InstalledReserveAction {
    pub mode: i32,
    pub amount: BigInt,
    pub loc_hash: String,
    pub loc_offset: i32,
}

pub struct InstalledSendMessageAction {
    pub msg_hash: String,
    pub msg_cell: Cell,
    pub loc_hash: String,
    pub loc_offset: i32,
}

impl InstalledSendMessageAction {
    pub fn message(&self) -> Option<RelaxedMessage> {
        let mut msg_slice = self.msg_cell.as_slice().ok()?;
        let msg = RelaxedMessage::load_from(&mut msg_slice).ok()?;
        Some(msg)
    }
}

pub fn find_installed_actions(vm_logs: &String) -> InstalledActions {
    let lines = vmlogs::parser::parse_lines(vm_logs.as_str());

    let actions = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| match line {
            Ok(VmLine::VmExecute { instr }) => {
                if *instr == "SENDRAWMSG" {
                    let stack_line = lines.get(idx - 2)?;
                    let VmLine::VmStack { stack } = stack_line.as_ref().ok()? else {
                        return None;
                    };
                    let loc_line = lines.get(idx - 1)?;
                    let VmLine::VmLoc { hash, offset } = loc_line.as_ref().ok()? else {
                        return None;
                    };
                    let parsed = stack.parsed();
                    if parsed.len() < 2 {
                        return None;
                    }
                    let Some(VmStackValue::Cell(CellLike::Cell(msg_cell))) =
                        parsed.get(parsed.len() - 2)
                    else {
                        return None;
                    };
                    let cell = Boc::decode_hex(msg_cell.to_string()).ok()?;

                    return Some(InstalledAction::Message(InstalledSendMessageAction {
                        msg_hash: cell.repr_hash().to_string().to_ascii_uppercase(),
                        msg_cell: cell,
                        loc_hash: hash.to_string(),
                        loc_offset: offset.to_string().parse().unwrap_or(0),
                    }));
                }

                if *instr == "RAWRESERVE" {
                    let stack_line = lines.get(idx - 2)?;
                    let VmLine::VmStack { stack } = stack_line.as_ref().ok()? else {
                        return None;
                    };
                    let loc_line = lines.get(idx - 1)?;
                    let VmLine::VmLoc { hash, offset } = loc_line.as_ref().ok()? else {
                        return None;
                    };
                    let parsed = stack.parsed();
                    if parsed.len() < 2 {
                        return None;
                    }
                    let Some(VmStackValue::Integer(mode)) = parsed.get(parsed.len() - 1) else {
                        return None;
                    };
                    let Some(VmStackValue::Integer(amount)) = parsed.get(parsed.len() - 2) else {
                        return None;
                    };
                    return Some(InstalledAction::Reserve(InstalledReserveAction {
                        mode: mode.parse().unwrap_or(0),
                        amount: amount.parse().unwrap_or(BigInt::zero()),
                        loc_hash: hash.to_string(),
                        loc_offset: offset.to_string().parse().unwrap_or(0),
                    }));
                }

                None
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    InstalledActions { actions }
}

pub enum ExecutedAction {
    SendMessage {
        hash: String,
        remaining_balance: BigInt,
    },
    ReserveCurrency {
        mode: i32,
        reserve: BigInt,
        balance: BigInt,
        original_balance: BigInt,
        changed_remaining_balance: BigInt,
        changed_reserved_balance: BigInt,
    },
}

pub fn extract_actions_from_executor_logs(logs: &String) -> Vec<ExecutedAction> {
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
                    changed_reserved_balance: BigInt::from(0),  // Will be updated by ChangedBalance
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

    actions
}
