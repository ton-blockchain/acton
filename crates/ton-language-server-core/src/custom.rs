use crate::Range;

/// The inferred type and the syntax range that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeAtPosition {
    pub type_name: String,
    pub range: Range,
}

impl TypeAtPosition {
    #[must_use]
    pub fn new(type_name: impl Into<String>, range: Range) -> Self {
        Self {
            type_name: type_name.into(),
            range,
        }
    }
}
