use crate::cell::*;
use crate::error::Error;
use crate::models::account::StorageUsedShort;
use crate::models::currency::CurrencyCollection;
use crate::num::*;

/// Storage phase info.
///
/// At this phase account pays for storing its state.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoragePhase {
    /// Amount of tokens collected for storing this contract for some time.
    pub storage_fees_collected: Tokens,
    /// Amount of tokens which this account owes to the network
    /// (if there was not enough balance to pay storage fee).
    pub storage_fees_due: Option<Tokens>,
    /// Account status change during execution of this phase.
    pub status_change: AccountStatusChange,
}

/// Credit phase info.
///
/// At this phase message balance is added to the account balance.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CreditPhase {
    /// Amount of tokens paid for the debt.
    pub due_fees_collected: Option<Tokens>,
    /// Amount of tokens added to the account balance from the remaining
    /// message balance.
    pub credit: CurrencyCollection,
}

/// Compute phase info.
///
/// At this phase the VM is executed to produce a list of actions.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum ComputePhase {
    /// Compute phase was skipped.
    Skipped(SkippedComputePhase),
    /// Compute phase was executed.
    Executed(ExecutedComputePhase),
}

impl Store for ComputePhase {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Skipped(phase) => {
                ok!(builder.store_bit_zero());
                phase.store_into(builder, context)
            }
            Self::Executed(phase) => {
                let cell = {
                    let mut builder = CellBuilder::new();
                    ok!(phase.gas_used.store_into(&mut builder, context));
                    ok!(phase.gas_limit.store_into(&mut builder, context));
                    ok!(phase.gas_credit.store_into(&mut builder, context));
                    ok!(builder.store_u8(phase.mode as u8));
                    ok!(builder.store_u32(phase.exit_code as u32));
                    ok!(phase.exit_arg.store_into(&mut builder, context));
                    ok!(builder.store_u32(phase.vm_steps));
                    ok!(builder.store_u256(&phase.vm_init_state_hash));
                    ok!(builder.store_u256(&phase.vm_final_state_hash));
                    ok!(builder.build_ext(context))
                };

                let flags = 0b1000u8
                    | ((phase.success as u8) << 2)
                    | ((phase.msg_state_used as u8) << 1)
                    | (phase.account_activated as u8);
                ok!(builder.store_small_uint(flags, 4));
                ok!(phase.gas_fees.store_into(builder, context));
                builder.store_reference(cell)
            }
        }
    }
}

impl<'a> Load<'a> for ComputePhase {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if !ok!(slice.load_bit()) {
            return Ok(Self::Skipped(ok!(SkippedComputePhase::load_from(slice))));
        }

        let flags = ok!(slice.load_small_uint(3));
        let gas_fees = ok!(Tokens::load_from(slice));

        let slice = &mut ok!(slice.load_reference_as_slice());
        Ok(Self::Executed(ExecutedComputePhase {
            success: flags & 0b100 != 0,
            msg_state_used: flags & 0b010 != 0,
            account_activated: flags & 0b001 != 0,
            gas_fees,
            gas_used: ok!(VarUint56::load_from(slice)),
            gas_limit: ok!(VarUint56::load_from(slice)),
            gas_credit: ok!(Option::<VarUint24>::load_from(slice)),
            mode: ok!(slice.load_u8()) as i8,
            exit_code: ok!(slice.load_u32()) as i32,
            exit_arg: ok!(Option::<i32>::load_from(slice)),
            vm_steps: ok!(slice.load_u32()),
            vm_init_state_hash: ok!(slice.load_u256()),
            vm_final_state_hash: ok!(slice.load_u256()),
        }))
    }
}

/// Executed compute phase info.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecutedComputePhase {
    /// Whether the execution was successful.
    pub success: bool,
    /// Whether the `init` from the incoming message was used.
    pub msg_state_used: bool,
    /// Whether the account state changed to `Active` during this phase.
    pub account_activated: bool,
    /// Total amount of tokens spent to execute this phase.
    pub gas_fees: Tokens,
    /// Amount of gas used by the VM to execute this phase.
    pub gas_used: VarUint56,
    /// Max gas amount which could be used.
    pub gas_limit: VarUint56,
    /// Max gas amount which could be used before accepting this transaction.
    pub gas_credit: Option<VarUint24>,
    /// Execution mode.
    pub mode: i8,
    /// VM exit code.
    pub exit_code: i32,
    /// Additional VM exit argument.
    pub exit_arg: Option<i32>,
    /// The number of VM steps it took to complete this phase.
    pub vm_steps: u32,
    /// Hash of the initial state of the VM.
    pub vm_init_state_hash: HashBytes,
    /// Hash of the VM state after executing this phase.
    pub vm_final_state_hash: HashBytes,
}

/// Skipped compute phase info.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SkippedComputePhase {
    /// The reason this step was skipped.
    pub reason: ComputePhaseSkipReason,
}

/// Enum with reasons for skipping compute phase.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ComputePhaseSkipReason {
    /// Contract doesn't have state to execute.
    NoState,
    /// Contract state is invalid.
    BadState,
    /// Not enough gas to execute compute phase.
    NoGas,
    /// Account was suspended by the config.
    Suspended,
}

impl Store for ComputePhaseSkipReason {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let (tag, bits) = match self {
            Self::NoState => (0b00, 2),
            Self::BadState => (0b01, 2),
            Self::NoGas => (0b10, 2),
            Self::Suspended => (0b110, 3),
        };
        builder.store_small_uint(tag, bits)
    }
}

impl<'a> Load<'a> for ComputePhaseSkipReason {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_small_uint(2) {
            Ok(0b00) => Ok(Self::NoState),
            Ok(0b01) => Ok(Self::BadState),
            Ok(0b10) => Ok(Self::NoGas),
            Ok(_) => {
                if ok!(slice.load_bit()) {
                    // 0b11 -> 1
                    Err(Error::InvalidTag)
                } else {
                    // 0b11 -> 0
                    Ok(Self::Suspended)
                }
            }
            Err(e) => Err(e),
        }
    }
}

/// Action phase info.
///
/// At this phase the list of actions from the compute phase
/// is converted into updates and outgoing messages.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActionPhase {
    /// Whether the execution was successful.
    pub success: bool,
    /// Whether the action list was valid.
    pub valid: bool,
    /// There were no funds to create an outgoing message.
    pub no_funds: bool,
    /// Account status change during execution of this phase.
    pub status_change: AccountStatusChange,
    /// Total forwarding fee for outgoing messages.
    pub total_fwd_fees: Option<Tokens>,
    /// Total fees for processing all actions.
    pub total_action_fees: Option<Tokens>,
    /// Result code of the phase.
    pub result_code: i32,
    /// Optional result argument of the phase.
    pub result_arg: Option<i32>,
    /// The total number of processed actions.
    pub total_actions: u16,
    /// The number of special actions (`ReserveCurrency`, `SetCode`, `ChangeLibrary`).
    pub special_actions: u16,
    /// The number of skipped actions.
    pub skipped_actions: u16,
    /// The number of outgoing messages created by the compute phase.
    pub messages_created: u16,
    /// The hash of the actions list.
    pub action_list_hash: HashBytes,
    /// The total number of unique cells (bits / refs) of produced messages.
    pub total_message_size: StorageUsedShort,
}

impl Store for ActionPhase {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let flags = ((self.success as u8) << 2) | ((self.valid as u8) << 1) | self.no_funds as u8;
        let counts = ((self.total_actions as u64) << 48)
            | ((self.special_actions as u64) << 32)
            | ((self.skipped_actions as u64) << 16)
            | self.messages_created as u64;

        ok!(builder.store_small_uint(flags, 3));
        ok!(self.status_change.store_into(builder, context));
        ok!(self.total_fwd_fees.store_into(builder, context));
        ok!(self.total_action_fees.store_into(builder, context));
        ok!(builder.store_u32(self.result_code as u32));
        ok!(self.result_arg.store_into(builder, context));
        ok!(builder.store_u64(counts));
        ok!(builder.store_u256(&self.action_list_hash));
        self.total_message_size.store_into(builder, context)
    }
}

impl<'a> Load<'a> for ActionPhase {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let flags = ok!(slice.load_small_uint(3));

        let status_change = ok!(AccountStatusChange::load_from(slice));
        let total_fwd_fees = ok!(Option::<Tokens>::load_from(slice));
        let total_action_fees = ok!(Option::<Tokens>::load_from(slice));
        let result_code = ok!(slice.load_u32()) as i32;
        let result_arg = ok!(Option::<i32>::load_from(slice));

        let counts = ok!(slice.load_u64());

        Ok(Self {
            success: flags & 0b100 != 0,
            valid: flags & 0b010 != 0,
            no_funds: flags & 0b001 != 0,
            status_change,
            total_fwd_fees,
            total_action_fees,
            result_code,
            result_arg,
            total_actions: (counts >> 48) as u16,
            special_actions: (counts >> 32) as u16,
            skipped_actions: (counts >> 16) as u16,
            messages_created: counts as u16,
            action_list_hash: ok!(slice.load_u256()),
            total_message_size: ok!(StorageUsedShort::load_from(slice)),
        })
    }
}

/// Bounce phase info.
///
/// At this stage some funds are returned back to the sender.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum BouncePhase {
    /// Default phase state.
    ///
    /// Probably unused.
    NegativeFunds,
    /// There were not enough funds to execute this phase.
    NoFunds(NoFundsBouncePhase),
    /// Bounce phase was executed.
    Executed(ExecutedBouncePhase),
}

impl Store for BouncePhase {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::NegativeFunds => builder.store_small_uint(0b00, 2),
            Self::NoFunds(phase) => {
                ok!(builder.store_small_uint(0b01, 2));
                phase.store_into(builder, context)
            }
            Self::Executed(phase) => {
                ok!(builder.store_bit_one());
                phase.store_into(builder, context)
            }
        }
    }
}

impl<'a> Load<'a> for BouncePhase {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(if ok!(slice.load_bit()) {
            match ExecutedBouncePhase::load_from(slice) {
                Ok(phase) => Self::Executed(phase),
                Err(e) => return Err(e),
            }
        } else if ok!(slice.load_bit()) {
            match NoFundsBouncePhase::load_from(slice) {
                Ok(phase) => Self::NoFunds(phase),
                Err(e) => return Err(e),
            }
        } else {
            Self::NegativeFunds
        })
    }
}

/// Skipped bounce phase info.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NoFundsBouncePhase {
    /// The total number of unique cells (bits / refs) of the bounced message.
    pub msg_size: StorageUsedShort,
    /// Required amount of tokens to send the bounced message.
    pub req_fwd_fees: Tokens,
}

/// Executed bounce phase info.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecutedBouncePhase {
    /// The total number of unique cells (bits / refs) of the bounced message.
    pub msg_size: StorageUsedShort,
    /// The part of fees which fo to the validators.
    pub msg_fees: Tokens,
    /// Message forwarding fee.
    pub fwd_fees: Tokens,
}

/// Account status change during transaction execution.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AccountStatusChange {
    /// Account status has not changed.
    Unchanged = 0b0,
    /// Account has been frozen.
    Frozen = 0b10,
    /// Account deleted.
    Deleted = 0b11,
}

impl Store for AccountStatusChange {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        if *self == Self::Unchanged {
            builder.store_bit_zero()
        } else {
            builder.store_small_uint(*self as u8, 2)
        }
    }
}

impl<'a> Load<'a> for AccountStatusChange {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(if !ok!(slice.load_bit()) {
            Self::Unchanged
        } else if ok!(slice.load_bit()) {
            Self::Deleted
        } else {
            Self::Frozen
        })
    }
}
