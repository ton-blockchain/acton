use crate::ast::bless_call_missing_safety_comment::BlessCallMissingSafetyComment;
use crate::ast::dangerous_send_mode_missing_safety_comment::DangerousSendModeMissingSafetyComment;
use crate::ast::{
    acton_import_in_contract, asm_function_missing_safety_comment, compiler_error,
    deprecated_symbol_use, duplicated_condition, field_init_can_be_folded,
    identical_conditional_branches, import_path_can_use_mappings, message_entity_naming,
    method_can_be_static, mutable_parameter_can_be_immutable, mutable_variable_can_be_immutable,
    name_case_checker, negated_is_type_can_use_not_is, no_bounce_handler, no_global_variables,
    pure_function_call_unused, reserve_mode_literal, send_mode_literal,
    several_not_null_assertions, unused_import, unused_variable, used_ignored_identifier,
    write_only_variable,
};
use crate::dfa::{divide_before_multiply, random_requires_initialization, unauthorized_access};
use serde::Serialize;
use std::fmt::Display;

#[derive(Debug, Copy, Clone, Serialize)]
pub enum FixAvailability {
    Sometimes,
    Always,
    None,
}

impl Display for FixAvailability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixAvailability::Sometimes => write!(f, "Fix is sometimes available."),
            FixAvailability::Always => write!(f, "Fix is always available."),
            FixAvailability::None => write!(f, "Fix is not available."),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum FromCodeError {
    #[error("unknown rule code")]
    Unknown,
}

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, Hash)]
pub enum Linter {
    Tolk,
}

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "status", content = "since", rename_all = "snake_case")]
pub enum RuleGroup {
    /// The rule is stable since the provided version.
    Stable { since: &'static str },
    /// The rule is in preview since the provided version.
    Preview { since: &'static str },
    /// The rule is deprecated since the provided version.
    Deprecated { since: &'static str },
    /// The rule was removed in the provided version.
    Removed { since: &'static str },
}

#[tolk_macros::map_codes]
pub fn code_to_rule(linter: Linter, code: &str) -> Option<(RuleGroup, Rule)> {
    use Linter::*;

    Some(match (linter, code) {
        (Tolk, "E001") => field_init_can_be_folded::FieldInitCanBeFolded,
        (Tolk, "E002") => unused_variable::UnusedVariable,
        (Tolk, "E003") => mutable_variable_can_be_immutable::MutableVariableCanBeImmutable,
        (Tolk, "E004") => deprecated_symbol_use::DeprecatedSymbolUse,
        (Tolk, "E005") => write_only_variable::WriteOnlyVariable,
        (Tolk, "E006") => unused_import::UnusedImport,
        (Tolk, "E007") => pure_function_call_unused::PureFunctionCallUnused,
        (Tolk, "E008") => no_bounce_handler::NoBounceHandler,
        (Tolk, "E009") => method_can_be_static::MethodCanBeStatic,
        (Tolk, "E010") => used_ignored_identifier::UsedIgnoredIdentifier,
        (Tolk, "E011") => message_entity_naming::MessageShouldBeNamed,
        (Tolk, "E012") => message_entity_naming::CreateMessageInlineSend,
        (Tolk, "E013") => mutable_parameter_can_be_immutable::MutableParameterCanBeImmutable,
        (Tolk, "E014") => acton_import_in_contract::ActonImportInContract,
        (Tolk, "E015") => asm_function_missing_safety_comment::AsmFunctionMissingSafetyComment,
        (Tolk, "E016") => send_mode_literal::SendModeLiteral,
        (Tolk, "E017") => unauthorized_access::UnauthorizedAccess,
        (Tolk, "E018") => import_path_can_use_mappings::ImportPathCanUseMappings,
        (Tolk, "E019") => several_not_null_assertions::SeveralNotNullAssertions,
        (Tolk, "E020") => reserve_mode_literal::ReserveModeLiteral,
        (Tolk, "E021") => DangerousSendModeMissingSafetyComment,
        (Tolk, "E022") => negated_is_type_can_use_not_is::NegatedIsTypeCanUseNotIs,
        (Tolk, "E023") => BlessCallMissingSafetyComment,
        (Tolk, "E024") => random_requires_initialization::RandomRequiresInitialization,
        (Tolk, "E025") => divide_before_multiply::DivideBeforeMultiply,
        (Tolk, "E026") => duplicated_condition::DuplicatedCondition,
        (Tolk, "E027") => identical_conditional_branches::IdenticalConditionalBranches,
        (Tolk, "E028") => no_global_variables::NoGlobalVariables,
        (Tolk, "C001") => compiler_error::CompilerError,
        (Tolk, "S001") => name_case_checker::NameCaseChecker,
        _ => return None,
    })
}

pub trait ViolationMetadata {
    /// Returns the rule for this violation
    fn rule() -> Rule;

    /// Returns the code for this violation
    fn code() -> Option<&'static str> {
        Linter::Tolk.code_for_rule(Self::rule())
    }

    /// Returns an explanation of what this violation catches,
    /// why it's bad, and what users should do instead.
    fn explain() -> Option<&'static str>;

    /// Returns the rule group for this violation.
    fn group() -> RuleGroup;

    /// Returns the file where the violation is declared.
    fn file() -> &'static str;

    /// Returns the 1-based line where the violation is declared.
    fn line() -> u32;
}

pub trait Violation: ViolationMetadata + Sized {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    /// The message used to describe the violation.
    fn message(&self) -> String;
}
