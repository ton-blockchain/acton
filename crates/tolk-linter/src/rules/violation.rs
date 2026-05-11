use crate::ast::bless_call_missing_safety_comment::BlessCallMissingSafetyComment;
use crate::ast::dangerous_send_mode_missing_safety_comment::DangerousSendModeMissingSafetyComment;
use crate::ast::enum_cast_missing_safety_comment::EnumCastMissingSafetyComment;
use crate::ast::{
    acton_import_in_contract, asm_function_missing_safety_comment, compiler_error,
    create_message_body_to_cell, deprecated_symbol_use, dict_type_use, duplicated_condition,
    explicit_return_type, field_init_can_be_folded, identical_conditional_branches,
    import_path_can_use_mappings, message_entity_naming, method_can_be_static,
    missing_contract_header, mutable_parameter_can_be_immutable, mutable_variable_can_be_immutable,
    name_case_checker, negated_is_type_can_use_not_is, no_bounce_handler, no_global_variables,
    pure_function_call_unused, reserve_mode_literal, send_mode_literal,
    several_not_null_assertions, throw_requires_documented_error_value, throw_requires_errors_enum,
    unused_expression, unused_import, unused_variable, used_ignored_identifier,
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
        (Tolk, "E001") => unused_variable::UnusedVariable,
        (Tolk, "E002") => mutable_variable_can_be_immutable::MutableVariableCanBeImmutable,
        (Tolk, "E003") => deprecated_symbol_use::DeprecatedSymbolUse,
        (Tolk, "E004") => write_only_variable::WriteOnlyVariable,
        (Tolk, "E005") => unused_import::UnusedImport,
        (Tolk, "E006") => pure_function_call_unused::PureFunctionCallUnused,
        (Tolk, "E007") => no_bounce_handler::NoBounceHandler,
        (Tolk, "E008") => used_ignored_identifier::UsedIgnoredIdentifier,
        (Tolk, "E009") => mutable_parameter_can_be_immutable::MutableParameterCanBeImmutable,
        (Tolk, "E010") => acton_import_in_contract::ActonImportInContract,
        (Tolk, "E011") => asm_function_missing_safety_comment::AsmFunctionMissingSafetyComment,
        (Tolk, "E012") => send_mode_literal::SendModeLiteral,
        (Tolk, "E013") => unauthorized_access::UnauthorizedAccess,
        (Tolk, "E014") => several_not_null_assertions::SeveralNotNullAssertions,
        (Tolk, "E015") => reserve_mode_literal::ReserveModeLiteral,
        (Tolk, "E016") => DangerousSendModeMissingSafetyComment,
        (Tolk, "E017") => BlessCallMissingSafetyComment,
        (Tolk, "E018") => random_requires_initialization::RandomRequiresInitialization,
        (Tolk, "E019") => divide_before_multiply::DivideBeforeMultiply,
        (Tolk, "E020") => duplicated_condition::DuplicatedCondition,
        (Tolk, "E021") => identical_conditional_branches::IdenticalConditionalBranches,
        (Tolk, "E022") => no_global_variables::NoGlobalVariables,
        (Tolk, "E024") => EnumCastMissingSafetyComment,
        (Tolk, "E025") => missing_contract_header::MissingContractHeader,
        (Tolk, "E026") => unused_expression::UnusedExpression,
        (Tolk, "E027") => dict_type_use::DictTypeUse,
        (Tolk, "E028") => throw_requires_errors_enum::ThrowRequiresErrorsEnum,
        (Tolk, "E029") => create_message_body_to_cell::CreateMessageBodyToCell,
        (Tolk, "E030") => throw_requires_documented_error_value::ThrowRequiresDocumentedErrorValue,
        (Tolk, "C001") => compiler_error::CompilerError,
        (Tolk, "S001") => name_case_checker::NameCaseChecker,
        (Tolk, "S002") => explicit_return_type::ExplicitReturnType,
        (Tolk, "S003") => field_init_can_be_folded::FieldInitCanBeFolded,
        (Tolk, "S004") => method_can_be_static::MethodCanBeStatic,
        (Tolk, "S005") => message_entity_naming::MessageShouldBeNamed,
        (Tolk, "S006") => message_entity_naming::CreateMessageInlineSend,
        (Tolk, "S007") => import_path_can_use_mappings::ImportPathCanUseMappings,
        (Tolk, "S008") => negated_is_type_can_use_not_is::NegatedIsTypeCanUseNotIs,
        _ => return None,
    })
}

pub trait ViolationMetadata {
    /// Returns the rule for this violation
    fn rule() -> Rule;

    /// Returns the code for this violation
    #[must_use]
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
