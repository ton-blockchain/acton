pub enum Rule {
    FieldInitCanBeFolded,
    UnusedVariable,
    MutableVariableCanBeImmutable,
}

pub trait ViolationMetadata {
    /// Returns the rule for this violation
    fn rule() -> Rule;

    /// Returns an explanation of what this violation catches,
    /// why it's bad, and what users should do instead.
    fn explain() -> Option<&'static str>;
}

pub trait Violation: ViolationMetadata + Sized {
    /// The message used to describe the violation.
    fn message(&self) -> String;
}
