use crate::ast::{
    deprecated_symbol_use, field_init_can_be_folded, missing_on_bounce_handler,
    mutable_variable_can_be_immutable, pure_function_call_unused, unused_import, unused_variable,
    write_only_variable,
};
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
        (Tolk, "E008") => missing_on_bounce_handler::MissingOnBounceHandler,
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
