use crate::FixAvailability;
use crate::rules::violation::Violation;
use tolk_macros::ViolationMetadata;

/// ### What it does
/// Reports compiler and parse errors.
///
/// ### Behavior notes
/// - This diagnostic is emitted outside per-rule lint processing.
///   Inline suppressions and `[lint.rules]` settings do not disable it.
/// - In machine-readable output, compiler errors use code `C001`; parse errors
///   are grouped under `compiler-error` but currently have no `code` field.
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct CompilerError;

impl Violation for CompilerError {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::None;

    fn message(&self) -> String {
        "compiler error".to_string()
    }
}
