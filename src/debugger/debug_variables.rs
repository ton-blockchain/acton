use crate::context::to_cell;
use crate::debugger::any_executor::AnyExecutor;
use crate::debugger::debug_context::{DebugContext, VARIABLE_REFERENCE_COUNTER};
use crate::formatter::FormatterContext;
use anyhow::anyhow;
use dap::requests::VariablesArguments;
use dap::types::Variable;
use log::debug;
use std::sync::atomic::Ordering;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::serde::parse_tuple_item;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::models::{
    CurrencyCollection, IntAddr, OutAction, OutActionsRevIter, OwnedRelaxedMessage, RelaxedMsgInfo,
    StateInit,
};
use tycho_types::num::Tokens;

impl DebugContext {
    pub fn process_variables(
        &mut self,
        args: &&VariablesArguments,
    ) -> anyhow::Result<Vec<Variable>> {
        debug!(
            "Processing variables request: variables_reference={}, count={:?}",
            args.variables_reference, args.count
        );
        let stepper = &self.stepper;

        let current_loc = match &stepper.get_current_step() {
            Some(step) => match &step.loc {
                Some(loc) => loc,
                None => return Ok(vec![]),
            },
            None => return Ok(vec![]),
        };

        let executor = &stepper.executors[stepper.current_executor_id];

        let variables = if args.variables_reference == 1 {
            let stack = executor.get_stack();
            let stack = Tuple::deserialize(&ArcCell::from_boc_b64(&stack)?)?;

            current_loc
                .variables
                .iter()
                .rev()
                .enumerate()
                .flat_map(|(index, variable)| {
                    if index >= stack.len() {
                        return None;
                    }

                    let value = stack
                        .get(stack.len() - 1 - index)
                        .unwrap_or(&TupleItem::Null);
                    let typed_value = value.to_typed(&variable.var_type);
                    Some(Variable {
                        name: variable.name.clone(),
                        type_field: Some(variable.var_type.clone()),
                        value: self.formatter_context.format(&typed_value),
                        ..Default::default()
                    })
                })
                .collect::<Vec<_>>()
        } else if args.variables_reference == 2 {
            let mut variables = Vec::new();

            // c4 register (storage)
            if let Ok(out_actions) = self.get_storage(executor) {
                variables.push(Variable {
                    name: "c4 (storage)".to_string(),
                    type_field: Some("storage".to_string()),
                    value: out_actions.to_boc_hex(false)?,
                    ..Default::default()
                });
            }

            // c5 register (out actions)
            if let Ok(out_actions) = self.get_out_actions(executor) {
                let c5_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                self.variables
                    .out_actions
                    .insert(c5_ref, out_actions.clone());

                variables.push(Variable {
                    name: "c5 (output actions)".to_string(),
                    type_field: Some("out_actions".to_string()),
                    value: format!("{} out actions", out_actions.len()),
                    variables_reference: c5_ref,
                    ..Default::default()
                });
            }

            // c7 register
            let c7 = executor.get_c7();
            let c7_cell = &ArcCell::from_boc_b64(&c7)?;
            let mut c7_slice = c7_cell.parser();
            let c7_tuple = parse_tuple_item(&mut c7_slice)?;
            let c7_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
            self.variables.tuple.insert(c7_ref, c7_tuple.clone());

            variables.push(Variable {
                name: "c7 (temporary data)".to_string(),
                type_field: Some("tuple".to_string()),
                value: self.formatter_context.format(&c7_tuple),
                variables_reference: c7_ref,
                ..Default::default()
            });

            variables
        } else if args.variables_reference == 3 {
            let stack_boc = executor.get_stack();
            let stack_cell = ArcCell::from_boc_b64(&stack_boc)?;
            let stack_tuple = Tuple::deserialize(&stack_cell)?;

            stack_tuple
                .iter()
                .rev()
                .enumerate()
                .map(|(index, item)| Variable {
                    name: format!("s{index}"),
                    type_field: Some("stack_item".to_string()),
                    value: self.formatter_context.format(item),
                    ..Default::default()
                })
                .collect::<Vec<_>>()
        } else if args.variables_reference > 3 {
            if let Some(tuple_item) = self.variables.tuple.get(&args.variables_reference) {
                self.build_tuple_children(&tuple_item.clone())
            } else if let Some(out_actions) =
                self.variables.out_actions.get(&args.variables_reference)
            {
                self.build_out_actions_children(&out_actions.clone())
            } else if let Some(out_action) =
                self.variables.out_action.get(&args.variables_reference)
            {
                self.build_out_action_children(&out_action.clone())
            } else if let Some(message) = self.variables.message.get(&args.variables_reference) {
                self.build_message_children(&message.clone())
            } else if let Some(msg_info) = self.variables.msg_info.get(&args.variables_reference) {
                self.build_msg_info_children(msg_info)
            } else if let Some(state_init) =
                self.variables.state_init.get(&args.variables_reference)
            {
                self.build_state_init_children(state_init)
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        debug!("Returning {} variables", variables.len());
        Ok(variables)
    }

    fn build_tuple_children(&mut self, tuple_item: &TupleItem) -> Vec<Variable> {
        match tuple_item {
            TupleItem::Tuple(items) => items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let item_ref = if Self::has_children(item) {
                        let ref_id =
                            VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                        self.variables.tuple.insert(ref_id, item.clone());
                        ref_id
                    } else {
                        0
                    };
                    Variable {
                        name: format!("[{index}]"),
                        type_field: Some(Self::get_item_type(item)),
                        value: self.formatter_context.format(item),
                        variables_reference: item_ref,
                        ..Default::default()
                    }
                })
                .collect(),
            TupleItem::TypedTuple { inner: items, .. } => items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let item_ref = if Self::has_children(item) {
                        let ref_id =
                            VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                        self.variables.tuple.insert(ref_id, item.clone());
                        ref_id
                    } else {
                        0
                    };
                    Variable {
                        name: format!("[{index}]"),
                        type_field: Some(Self::get_item_type(item)),
                        value: self.formatter_context.format(item),
                        variables_reference: item_ref,
                        ..Default::default()
                    }
                })
                .collect(),
            _ => vec![],
        }
    }

    fn has_children(item: &TupleItem) -> bool {
        matches!(item, TupleItem::Tuple(_) | TupleItem::TypedTuple { .. })
    }

    fn get_item_type(item: &TupleItem) -> String {
        match item {
            TupleItem::Null => "null".to_string(),
            TupleItem::Int(_) => "int".to_string(),
            TupleItem::Nan => "nan".to_string(),
            TupleItem::Cell(_) => "cell".to_string(),
            TupleItem::Slice(_) => "slice".to_string(),
            TupleItem::Builder(_) => "builder".to_string(),
            TupleItem::Tuple(_) => "tuple".to_string(),
            TupleItem::TypedTuple { type_name, .. } => type_name.clone(),
        }
    }

    fn get_storage(&self, executor: &AnyExecutor) -> anyhow::Result<ArcCell> {
        let c4 = executor.get_control_register(4);
        let c4_cell = &ArcCell::from_boc_b64(&c4)?;
        let mut c4_slice = c4_cell.parser();

        if let TupleItem::Cell(c4_tuple) = parse_tuple_item(&mut c4_slice)? {
            Ok(c4_tuple)
        } else {
            Ok(ArcCell::default())
        }
    }

    fn get_out_actions(&self, executor: &AnyExecutor) -> anyhow::Result<Vec<OutAction>> {
        let c5 = executor.get_control_register(5);
        let c5_cell = &ArcCell::from_boc_b64(&c5)?;
        let mut c5_slice = c5_cell.parser();

        if let TupleItem::Cell(c5_tuple) = parse_tuple_item(&mut c5_slice)? {
            let c5_boc = c5_tuple
                .to_boc(false)
                .map_err(|e| anyhow!("Failed to encode c5 tuple to BoC: {e}"))?;
            let c5_cell =
                &Boc::decode(&c5_boc).map_err(|e| anyhow!("Failed to decode c5 BoC: {e}"))?;
            let c5_slice = c5_cell.as_slice()?;

            let out_actions = OutActionsRevIter::new(c5_slice)
                .filter_map(|action| action.ok())
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .cloned()
                .collect();

            Ok(out_actions)
        } else {
            Ok(vec![])
        }
    }

    fn build_out_actions_children(&mut self, out_actions: &[OutAction]) -> Vec<Variable> {
        out_actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let action_type = match action {
                    OutAction::SendMsg { .. } => "SendMsg",
                    OutAction::SetCode { .. } => "SetCode",
                    OutAction::ReserveCurrency { .. } => "ReserveCurrency",
                    OutAction::ChangeLibrary { .. } => "ChangeLibrary",
                };

                let action_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                self.variables.out_action.insert(action_ref, action.clone());

                let value = match action {
                    OutAction::SendMsg { mode, out_msg } => {
                        if let Ok(message) = out_msg.load() {
                            format!(
                                "{} and {}",
                                Self::format_relaxed_msg_info(&message.info),
                                FormatterContext::format_send_msg_flags(*mode)
                            )
                        } else {
                            FormatterContext::format_send_msg_flags(*mode)
                        }
                    }
                    OutAction::ReserveCurrency { mode, value } => {
                        format!(
                            "{} with {}",
                            Self::format_currency_collection(value),
                            FormatterContext::format_reserve_currency_flags(*mode)
                        )
                    }
                    _ => format!("{action:?}"),
                };

                Variable {
                    name: format!("[{index}] {action_type}"),
                    type_field: Some(action_type.to_string()),
                    value,
                    variables_reference: action_ref,
                    ..Default::default()
                }
            })
            .collect()
    }

    fn build_out_action_children(&mut self, out_action: &OutAction) -> Vec<Variable> {
        match out_action {
            OutAction::SendMsg { mode, out_msg } => {
                let mut variables = vec![Variable {
                    name: "mode".to_string(),
                    type_field: Some("SendMsgFlags".to_string()),
                    value: FormatterContext::format_send_msg_flags(*mode),
                    ..Default::default()
                }];

                let message_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                if let Ok(message) = out_msg.load() {
                    self.variables.message.insert(message_ref, message);
                    variables.push(Variable {
                        name: "out_msg".to_string(),
                        type_field: Some("OwnedRelaxedMessage".to_string()),
                        value: "RelaxedMessage".to_string(),
                        variables_reference: message_ref,
                        ..Default::default()
                    });
                } else {
                    variables.push(Variable {
                        name: "out_msg".to_string(),
                        type_field: Some("Lazy<OwnedRelaxedMessage>".to_string()),
                        value: format!("{out_msg:?}"),
                        ..Default::default()
                    });
                }

                variables.push(Variable {
                    name: "out_msg_raw".to_string(),
                    type_field: Some("cell".to_string()),
                    value: Boc::encode_hex(out_msg.inner()),
                    ..Default::default()
                });

                variables
            }
            OutAction::SetCode { new_code } => vec![Variable {
                name: "new_code".to_string(),
                type_field: Some("Cell".to_string()),
                value: format!("{new_code:?}"),
                ..Default::default()
            }],
            OutAction::ReserveCurrency { mode, value } => vec![
                Variable {
                    name: "mode".to_string(),
                    type_field: Some("ReserveCurrencyFlags".to_string()),
                    value: FormatterContext::format_reserve_currency_flags(*mode),
                    ..Default::default()
                },
                Variable {
                    name: "value".to_string(),
                    type_field: Some("CurrencyCollection".to_string()),
                    value: Self::format_currency_collection(value),
                    ..Default::default()
                },
            ],
            OutAction::ChangeLibrary { mode, lib } => vec![
                Variable {
                    name: "mode".to_string(),
                    type_field: Some("ChangeLibraryMode".to_string()),
                    value: format!("{mode:?}"),
                    ..Default::default()
                },
                Variable {
                    name: "lib".to_string(),
                    type_field: Some("LibRef".to_string()),
                    value: format!("{lib:?}"),
                    ..Default::default()
                },
            ],
        }
    }

    fn format_tokens(tokens: &Tokens) -> String {
        format!("{:.9} TON", tokens.into_inner() as f64 / 1_000_000_000.0)
    }

    fn format_int_addr(addr: &IntAddr) -> String {
        match addr {
            IntAddr::Std(std_addr) => std_addr.display_base64(true).to_string(),
            IntAddr::Var(var_addr) => format!("{var_addr:?}"), // fallback for VarAddr
        }
    }

    fn format_relaxed_msg_info(info: &RelaxedMsgInfo) -> String {
        match info {
            RelaxedMsgInfo::Int(int_info) => {
                format!(
                    "to {} with {}",
                    Self::format_int_addr(&int_info.dst),
                    Self::format_currency_collection(&int_info.value)
                )
            }
            RelaxedMsgInfo::ExtOut(ext_info) => {
                format!(
                    "to {}",
                    ext_info
                        .dst
                        .as_ref()
                        .map_or("None".to_string(), |addr| addr.to_string())
                )
            }
        }
    }

    fn format_currency_collection(currency: &CurrencyCollection) -> String {
        let ton_amount = currency.tokens.into_inner() as f64 / 1_000_000_000.0;

        let mut result = format!("{ton_amount:.9} TON ");

        if !currency.other.is_empty() {
            let mut other_currencies = Vec::new();
            let dict = currency.other.as_dict();
            for (currency_id, amount) in dict.iter().flatten() {
                other_currencies.push(format!("{currency_id}: {amount}"));
            }
            if !other_currencies.is_empty() {
                result.push_str(&format!(" + [{}]", other_currencies.join(", ")));
            }
        }

        result
    }

    fn build_message_children(&mut self, message: &OwnedRelaxedMessage) -> Vec<Variable> {
        let mut variables = Vec::new();

        let info_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
        self.variables
            .msg_info
            .insert(info_ref, message.info.clone());
        variables.push(Variable {
            name: "info".to_string(),
            type_field: Some("RelaxedMsgInfo".to_string()),
            value: "RelaxedMsgInfo".to_string(),
            variables_reference: info_ref,
            ..Default::default()
        });

        if let Some(init) = &message.init {
            let init_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
            self.variables.state_init.insert(init_ref, init.clone());
            variables.push(Variable {
                name: "init".to_string(),
                type_field: Some("StateInit".to_string()),
                value: "StateInit".to_string(),
                variables_reference: init_ref,
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "init".to_string(),
                type_field: Some("Option<StateInit>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        let msg_cell = message.body.1.clone();
        let msg_offset = message.body.0.offset();
        let Ok(mut msg_slice) = msg_cell.as_slice() else {
            return Vec::new();
        };
        msg_slice.skip_first(msg_offset.bits, msg_offset.refs).ok();
        let msg_cell = to_cell(&msg_slice);

        variables.push(Variable {
            name: "body".to_string(),
            type_field: Some("CellSliceParts".to_string()),
            value: Boc::encode_hex(msg_cell),
            ..Default::default()
        });

        variables
    }

    fn build_msg_info_children(&self, msg_info: &RelaxedMsgInfo) -> Vec<Variable> {
        match msg_info {
            RelaxedMsgInfo::Int(int_info) => vec![
                Variable {
                    name: "ihr_disabled".to_string(),
                    type_field: Some("bool".to_string()),
                    value: int_info.ihr_disabled.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "bounce".to_string(),
                    type_field: Some("bool".to_string()),
                    value: int_info.bounce.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "bounced".to_string(),
                    type_field: Some("bool".to_string()),
                    value: int_info.bounced.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "src".to_string(),
                    type_field: Some("Option<IntAddr>".to_string()),
                    value: match &int_info.src {
                        Some(addr) => Self::format_int_addr(addr),
                        None => "None".to_string(),
                    },
                    ..Default::default()
                },
                Variable {
                    name: "dst".to_string(),
                    type_field: Some("IntAddr".to_string()),
                    value: Self::format_int_addr(&int_info.dst),
                    ..Default::default()
                },
                Variable {
                    name: "value".to_string(),
                    type_field: Some("CurrencyCollection".to_string()),
                    value: Self::format_currency_collection(&int_info.value),
                    ..Default::default()
                },
                Variable {
                    name: "ihr_fee".to_string(),
                    type_field: Some("Tokens".to_string()),
                    value: Self::format_tokens(&int_info.ihr_fee),
                    ..Default::default()
                },
                Variable {
                    name: "fwd_fee".to_string(),
                    type_field: Some("Tokens".to_string()),
                    value: Self::format_tokens(&int_info.fwd_fee),
                    ..Default::default()
                },
                Variable {
                    name: "created_lt".to_string(),
                    type_field: Some("u64".to_string()),
                    value: int_info.created_lt.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "created_at".to_string(),
                    type_field: Some("u32".to_string()),
                    value: int_info.created_at.to_string(),
                    ..Default::default()
                },
            ],
            RelaxedMsgInfo::ExtOut(ext_info) => vec![
                Variable {
                    name: "src".to_string(),
                    type_field: Some("Option<IntAddr>".to_string()),
                    value: match &ext_info.src {
                        Some(addr) => Self::format_int_addr(addr),
                        None => "None".to_string(),
                    },
                    ..Default::default()
                },
                Variable {
                    name: "dst".to_string(),
                    type_field: Some("Option<ExtAddr>".to_string()),
                    value: match &ext_info.dst {
                        Some(addr) => addr.to_string(),
                        None => "None".to_string(),
                    },
                    ..Default::default()
                },
                Variable {
                    name: "created_lt".to_string(),
                    type_field: Some("u64".to_string()),
                    value: ext_info.created_lt.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "created_at".to_string(),
                    type_field: Some("u32".to_string()),
                    value: ext_info.created_at.to_string(),
                    ..Default::default()
                },
            ],
        }
    }

    fn build_state_init_children(&self, state_init: &StateInit) -> Vec<Variable> {
        let mut variables = Vec::new();

        if let Some(split_depth) = &state_init.split_depth {
            variables.push(Variable {
                name: "split_depth".to_string(),
                type_field: Some("SplitDepth".to_string()),
                value: format!("{split_depth:?}"),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "split_depth".to_string(),
                type_field: Some("Option<SplitDepth>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        if let Some(special) = &state_init.special {
            variables.push(Variable {
                name: "special".to_string(),
                type_field: Some("SpecialFlags".to_string()),
                value: format!("tick: {}, tock: {}", special.tick, special.tock),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "special".to_string(),
                type_field: Some("Option<SpecialFlags>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        if let Some(code) = &state_init.code {
            variables.push(Variable {
                name: "code".to_string(),
                type_field: Some("Cell".to_string()),
                value: Boc::encode_hex(code),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "code".to_string(),
                type_field: Some("Option<Cell>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        if let Some(data) = &state_init.data {
            variables.push(Variable {
                name: "data".to_string(),
                type_field: Some("Cell".to_string()),
                value: Boc::encode_hex(data),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "data".to_string(),
                type_field: Some("Option<Cell>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        variables.push(Variable {
            name: "libraries".to_string(),
            type_field: Some("Dict<HashBytes, SimpleLib>".to_string()),
            value: if state_init.libraries.is_empty() {
                "empty".to_string()
            } else {
                format!(
                    "{} libraries",
                    state_init.libraries.iter().collect::<Vec<_>>().len()
                )
            },
            ..Default::default()
        });

        variables
    }
}
