use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::test::MutationLevel;

/// JSON file format accepted by `acton test --mutation-rules-file`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[schemars(title = "Acton Custom Mutation Rules Schema")]
pub enum CustomMutationRulesFile {
    /// A bare array of custom mutation rules.
    Bare(Vec<CustomMutationRule>),
    /// A wrapper object containing custom mutation rules.
    Wrapped {
        /// Custom mutation rules.
        rules: Vec<CustomMutationRule>,
    },
}

impl CustomMutationRulesFile {
    #[must_use]
    pub fn into_rules(self) -> Vec<CustomMutationRule> {
        match self {
            Self::Bare(rules) | Self::Wrapped { rules } => rules,
        }
    }
}

/// Query-based mutation rule loaded from a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CustomMutationRule {
    /// Unique rule ID. Custom rules override built-in rules with the same ID.
    pub name: String,
    /// Short human-readable rule description.
    pub description: String,
    /// Explanation shown when this mutation survives.
    pub explanation: String,
    /// Mutation level used by mutation level filters.
    pub level: MutationLevel,
    /// Rule group used in mutation reports.
    pub group: String,
    /// Tree-sitter matcher that selects source ranges to mutate.
    pub matcher: CustomMutationMatcher,
    /// Edit applied to each matched source range.
    pub edit: CustomMutationEdit,
}

/// Serializable matcher for custom mutation rules.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CustomMutationMatcher {
    /// Tree-sitter query matcher.
    Query {
        /// Tree-sitter query run against Tolk source.
        query: String,
        /// Capture name whose node span should be mutated.
        capture: String,
    },
}

/// Serializable edit for custom mutation rules.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CustomMutationEdit {
    /// Remove the matched source range.
    Remove,
    /// Replace the matched source range with a literal string.
    Replace {
        /// Replacement text.
        replacement: String,
    },
}
