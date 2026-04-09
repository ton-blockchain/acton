use anyhow::Context;
use path_absolutize::Absolutize;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tree_sitter::Node;

pub(super) fn rules() -> Vec<MutationRule> {
    vec![
        MutationRule::remove(
            "remove_assert",
            "Remove assert statements",
            "This assertion is not covered by tests. This could lead to security vulnerabilities if the condition is not enforced.",
            MutationLevel::Critical,
            "assertion",
            MutationMatcher::Query {
                query: r"(assert_statement) @assert",
                capture: "assert",
            },
        ),
        MutationRule::remove(
            "remove_throw",
            "Remove throw keyword",
            "This exception path is not covered by tests. Missing error handling might leave the contract in an inconsistent state.",
            MutationLevel::Critical,
            "assertion",
            MutationMatcher::Callback {
                predicate: |node, _| -> anyhow::Result<bool> {
                    if node.kind() != "throw" {
                        return Ok(false);
                    }
                    let parent_kind = node.parent().map_or("", |p| p.kind());
                    Ok(parent_kind != "assert_statement")
                },
            },
        ),
        MutationRule::remove(
            "remove_storage_save_call",
            "Remove storage save() method calls",
            "This storage save operation is not covered by tests. Without this call, data changes won't be persisted to storage.",
            MutationLevel::Critical,
            "storage",
            MutationMatcher::Query {
                query: r#"(function_call callee: (dot_access obj: (identifier) field: (identifier) @method) (#eq? @method "save")) @call"#,
                capture: "call",
            },
        ),
        MutationRule::remove(
            "remove_set_data_call",
            "Remove contract.setData() method calls",
            "This storage write is not covered by tests. Without contract.setData(...), updated state will not be persisted.",
            MutationLevel::Critical,
            "storage",
            MutationMatcher::Query {
                query: r#"(function_call callee: (dot_access obj: (identifier) @obj field: (identifier) @method) (#eq? @obj "contract") (#eq? @method "setData")) @call"#,
                capture: "call",
            },
        ),
        MutationRule::remove(
            "remove_accept_external_message",
            "Remove acceptExternalMessage() calls",
            "This external accept path is not covered by tests. Without acceptExternalMessage(), message handling semantics may break.",
            MutationLevel::Critical,
            "external",
            MutationMatcher::Query {
                query: r#"(function_call callee: (identifier) @fn (#eq? @fn "acceptExternalMessage")) @call"#,
                capture: "call",
            },
        ),
        MutationRule::remove(
            "remove_commit_contract_data_and_actions",
            "Remove commitContractDataAndActions() calls",
            "This commit path is not covered by tests. Without commitContractDataAndActions(), state persistence and replay-protection semantics may break.",
            MutationLevel::Critical,
            "external",
            MutationMatcher::Query {
                query: r#"(function_call callee: (identifier) @fn (#eq? @fn "commitContractDataAndActions")) @call"#,
                capture: "call",
            },
        ),
        MutationRule::remove(
            "remove_set_code_postponed",
            "Remove contract.setCodePostponed() method calls",
            "This upgrade path is not covered by tests. Without contract.setCodePostponed(...), code-upgrade and action-isolation semantics may break.",
            MutationLevel::Critical,
            "upgrade",
            MutationMatcher::Query {
                query: r#"(function_call callee: (dot_access obj: (identifier) @obj field: (identifier) @method) (#eq? @obj "contract") (#eq? @method "setCodePostponed")) @call"#,
                capture: "call",
            },
        ),
        MutationRule::replace(
            "flip_plus",
            "Replace + with -",
            "This arithmetic operation is not fully covered by tests. Changing + to - did not cause any tests to fail.",
            MutationLevel::Major,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "+" @op)"#,
                capture: "op",
            },
            "-",
        ),
        MutationRule::replace(
            "flip_minus",
            "Replace - with +",
            "This arithmetic operation is not fully covered by tests. Changing - to + did not cause any tests to fail.",
            MutationLevel::Major,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "-" @op)"#,
                capture: "op",
            },
            "+",
        ),
        MutationRule::replace(
            "flip_mul_div",
            "Replace * with /",
            "This arithmetic operation is not fully covered by tests. Changing * to / did not cause any tests to fail.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "*" @op)"#,
                capture: "op",
            },
            "/",
        ),
        MutationRule::replace(
            "flip_div_mul",
            "Replace / with *",
            "This arithmetic operation is not fully covered by tests. Changing / to * did not cause any tests to fail.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "/" @op)"#,
                capture: "op",
            },
            "*",
        ),
        MutationRule::replace(
            "flip_eq_ne",
            "Replace == with !=",
            "This comparison is not fully covered by tests. Inverting the equality check did not cause any tests to fail.",
            MutationLevel::Major,
            "comparison",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "==" @op)"#,
                capture: "op",
            },
            "!=",
        ),
        MutationRule::replace(
            "flip_ne_eq",
            "Replace != with ==",
            "This comparison is not fully covered by tests. Inverting the inequality check did not cause any tests to fail.",
            MutationLevel::Major,
            "comparison",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "!=" @op)"#,
                capture: "op",
            },
            "==",
        ),
        MutationRule::replace(
            "flip_lt_le",
            "Replace < with <=",
            "This comparison boundary is not strictly checked. Changing < to <= did not affect test results.",
            MutationLevel::Major,
            "comparison",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "<" @op)"#,
                capture: "op",
            },
            "<=",
        ),
        MutationRule::replace(
            "flip_gt_ge",
            "Replace > with >=",
            "This comparison boundary is not strictly checked. Changing > to >= did not affect test results.",
            MutationLevel::Major,
            "comparison",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: ">" @op)"#,
                capture: "op",
            },
            ">=",
        ),
        MutationRule::replace(
            "flip_le_lt",
            "Replace <= with <",
            "This comparison boundary is not strictly checked. Changing <= to < did not affect test results.",
            MutationLevel::Major,
            "comparison",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "<=" @op)"#,
                capture: "op",
            },
            "<",
        ),
        MutationRule::replace(
            "flip_ge_gt",
            "Replace >= with >",
            "This comparison boundary is not strictly checked. Changing >= to > did not affect test results.",
            MutationLevel::Major,
            "comparison",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: ">=" @op)"#,
                capture: "op",
            },
            ">",
        ),
        MutationRule::replace(
            "invert_bool_true",
            "Replace true with false",
            "This boolean logic is not fully covered. Replacing 'true' with 'false' did not fail any tests.",
            MutationLevel::Major,
            "boolean",
            MutationMatcher::Query {
                query: r#"((boolean_literal) @true (#eq? @true "true"))"#,
                capture: "true",
            },
            "false",
        ),
        MutationRule::replace(
            "invert_bool_false",
            "Replace false with true",
            "This boolean logic is not fully covered. Replacing 'false' with 'true' did not fail any tests.",
            MutationLevel::Major,
            "boolean",
            MutationMatcher::Query {
                query: r#"((boolean_literal) @false (#eq? @false "false"))"#,
                capture: "false",
            },
            "true",
        ),
        MutationRule::replace(
            "flip_plus_assign",
            "Replace += with -=",
            "This compound assignment is not fully covered. Changing += to -= did not affect test results.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "+=" @op)"#,
                capture: "op",
            },
            "-=",
        ),
        MutationRule::replace(
            "flip_minus_assign",
            "Replace -= with +=",
            "This compound assignment is not fully covered. Changing -= to += did not affect test results.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "-=" @op)"#,
                capture: "op",
            },
            "+=",
        ),
        MutationRule::replace(
            "flip_logical_and",
            "Replace && with ||",
            "This logical operation is not fully covered. Changing && to || did not affect test results.",
            MutationLevel::Major,
            "logical",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "&&" @op)"#,
                capture: "op",
            },
            "||",
        ),
        MutationRule::replace(
            "flip_logical_or",
            "Replace || with &&",
            "This logical operation is not fully covered. Changing || to && did not affect test results.",
            MutationLevel::Major,
            "logical",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "||" @op)"#,
                capture: "op",
            },
            "&&",
        ),
        MutationRule::replace(
            "flip_bitwise_and",
            "Replace & with |",
            "This bitwise operation is not fully covered. Changing & to | did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "&" @op)"#,
                capture: "op",
            },
            "|",
        ),
        MutationRule::replace(
            "flip_bitwise_or",
            "Replace | with &",
            "This bitwise operation is not fully covered. Changing | to & did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "|" @op)"#,
                capture: "op",
            },
            "&",
        ),
        MutationRule::replace(
            "flip_bitwise_and_xor",
            "Replace & with ^",
            "This bitwise operation is not fully covered. Changing & to ^ did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "&" @op)"#,
                capture: "op",
            },
            "^",
        ),
        MutationRule::replace(
            "flip_bitwise_or_xor",
            "Replace | with ^",
            "This bitwise operation is not fully covered. Changing | to ^ did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "|" @op)"#,
                capture: "op",
            },
            "^",
        ),
        MutationRule::replace(
            "flip_bitwise_xor",
            "Replace ^ with &",
            "This bitwise operation is not fully covered. Changing ^ to & did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "^" @op)"#,
                capture: "op",
            },
            "&",
        ),
        MutationRule::replace(
            "flip_bitwise_xor_or",
            "Replace ^ with |",
            "This bitwise operation is not fully covered. Changing ^ to | did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "^" @op)"#,
                capture: "op",
            },
            "|",
        ),
        MutationRule::replace(
            "flip_bitwise_and_assign_or",
            "Replace &= with |=",
            "This compound bitwise assignment is not fully covered. Changing &= to |= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "&=" @op)"#,
                capture: "op",
            },
            "|=",
        ),
        MutationRule::replace(
            "flip_bitwise_and_assign_xor",
            "Replace &= with ^=",
            "This compound bitwise assignment is not fully covered. Changing &= to ^= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "&=" @op)"#,
                capture: "op",
            },
            "^=",
        ),
        MutationRule::replace(
            "flip_bitwise_or_assign_and",
            "Replace |= with &=",
            "This compound bitwise assignment is not fully covered. Changing |= to &= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "|=" @op)"#,
                capture: "op",
            },
            "&=",
        ),
        MutationRule::replace(
            "flip_bitwise_or_assign_xor",
            "Replace |= with ^=",
            "This compound bitwise assignment is not fully covered. Changing |= to ^= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "|=" @op)"#,
                capture: "op",
            },
            "^=",
        ),
        MutationRule::replace(
            "flip_bitwise_xor_assign_and",
            "Replace ^= with &=",
            "This compound bitwise assignment is not fully covered. Changing ^= to &= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "^=" @op)"#,
                capture: "op",
            },
            "&=",
        ),
        MutationRule::replace(
            "flip_bitwise_xor_assign_or",
            "Replace ^= with |=",
            "This compound bitwise assignment is not fully covered. Changing ^= to |= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "^=" @op)"#,
                capture: "op",
            },
            "|=",
        ),
        MutationRule::replace(
            "flip_lshift_assign",
            "Replace <<= with >>=",
            "This compound shift assignment is not fully covered. Changing <<= to >>= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "<<=" @op)"#,
                capture: "op",
            },
            ">>=",
        ),
        MutationRule::replace(
            "flip_rshift_assign",
            "Replace >>= with <<=",
            "This compound shift assignment is not fully covered. Changing >>= to <<= did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: ">>=" @op)"#,
                capture: "op",
            },
            "<<=",
        ),
        MutationRule::replace(
            "flip_lshift",
            "Replace << with >>",
            "This bitwise shift is not fully covered. Changing << to >> did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: "<<" @op)"#,
                capture: "op",
            },
            ">>",
        ),
        MutationRule::replace(
            "flip_rshift",
            "Replace >> with <<",
            "This bitwise shift is not fully covered. Changing >> to << did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(binary_operator operator_name: ">>" @op)"#,
                capture: "op",
            },
            "<<",
        ),
        MutationRule::replace(
            "remove_logical_not",
            "Remove logical NOT (!)",
            "The logical negation is not fully covered. Removing '!' did not affect test results.",
            MutationLevel::Critical,
            "critical",
            MutationMatcher::Query {
                query: r#"(unary_operator operator_name: "!" @op)"#,
                capture: "op",
            },
            "",
        ),
        MutationRule::replace(
            "remove_bitwise_not",
            "Remove bitwise NOT (~)",
            "The bitwise negation is not fully covered. Removing '~' did not affect test results.",
            MutationLevel::Minor,
            "bitwise",
            MutationMatcher::Query {
                query: r#"(unary_operator operator_name: "~" @op)"#,
                capture: "op",
            },
            "",
        ),
        MutationRule::replace(
            "flip_mul_assign",
            "Replace *= with /=",
            "This compound assignment is not fully covered. Changing *= to /= did not affect test results.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "*=" @op)"#,
                capture: "op",
            },
            "/=",
        ),
        MutationRule::replace(
            "flip_div_assign",
            "Replace /= with *=",
            "This compound assignment is not fully covered. Changing /= to *= did not affect test results.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "/=" @op)"#,
                capture: "op",
            },
            "*=",
        ),
        MutationRule::replace(
            "flip_mul_assign_mod",
            "Replace *= with %=",
            "This compound assignment is not fully covered. Changing *= to %= did not affect test results.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "*=" @op)"#,
                capture: "op",
            },
            "%=",
        ),
        MutationRule::replace(
            "flip_mod_assign",
            "Replace %= with *=",
            "This compound assignment is not fully covered. Changing %= to *= did not affect test results.",
            MutationLevel::Minor,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "%=" @op)"#,
                capture: "op",
            },
            "*=",
        ),
        MutationRule::replace(
            "remove_unary_minus",
            "Remove unary minus (-)",
            "The unary negation is not fully covered. Removing '-' did not affect test results.",
            MutationLevel::Major,
            "arithmetic",
            MutationMatcher::Query {
                query: r#"(unary_operator operator_name: "-" @op)"#,
                capture: "op",
            },
            "",
        ),
        MutationRule::replace(
            "if_condition_true",
            "Replace if condition with true",
            "The conditional logic is not fully covered. Forcing the condition to true did not affect test results.",
            MutationLevel::Critical,
            "control-flow",
            MutationMatcher::Query {
                query: r"(if_statement condition: (_) @cond)",
                capture: "cond",
            },
            "true",
        ),
        MutationRule::replace(
            "if_condition_false",
            "Replace if condition with false",
            "The conditional logic is not fully covered. Forcing the condition to false did not affect test results.",
            MutationLevel::Critical,
            "control-flow",
            MutationMatcher::Query {
                query: r"(if_statement condition: (_) @cond)",
                capture: "cond",
            },
            "false",
        ),
        MutationRule::replace(
            "while_condition_false",
            "Replace while condition with false",
            "The loop execution path is not fully covered. Forcing the loop condition to false did not affect test results.",
            MutationLevel::Critical,
            "control-flow",
            MutationMatcher::Query {
                query: r"(while_statement condition: (_) @cond)",
                capture: "cond",
            },
            "false",
        ),
    ]
}

#[derive(Clone, Debug)]
pub(super) enum MutationEdit {
    Remove,
    Replace { replacement: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum MutationLevel {
    Critical,
    Major,
    Minor,
}

impl MutationLevel {
    pub(crate) const fn label(&self) -> &'static str {
        match self {
            MutationLevel::Critical => "critical",
            MutationLevel::Major => "major",
            MutationLevel::Minor => "minor",
        }
    }
}

pub(super) type NodePredicate = for<'a> fn(Node<'a>, &str) -> anyhow::Result<bool>;

#[derive(Clone, Debug)]
pub(super) enum MutationMatcher {
    Query {
        query: &'static str,
        capture: &'static str,
    },
    Callback {
        predicate: NodePredicate,
    },
}

#[derive(Clone, Debug)]
pub(super) struct MutationRule {
    pub name: String,
    pub description: String,
    pub explanation: String,
    pub level: MutationLevel,
    pub group: String,
    pub edit: MutationEdit,
    pub matcher: MutationMatcher,
}

impl MutationRule {
    fn remove(
        name: impl Into<String>,
        description: impl Into<String>,
        explanation: impl Into<String>,
        level: MutationLevel,
        group: impl Into<String>,
        matcher: MutationMatcher,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            explanation: explanation.into(),
            level,
            group: group.into(),
            edit: MutationEdit::Remove,
            matcher,
        }
    }

    fn replace(
        name: impl Into<String>,
        description: impl Into<String>,
        explanation: impl Into<String>,
        level: MutationLevel,
        group: impl Into<String>,
        matcher: MutationMatcher,
        replacement: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            explanation: explanation.into(),
            level,
            group: group.into(),
            edit: MutationEdit::Replace {
                replacement: replacement.into(),
            },
            matcher,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MutationRulesFile {
    Bare(Vec<CustomMutationRule>),
    Wrapped { rules: Vec<CustomMutationRule> },
}

impl MutationRulesFile {
    fn into_rules(self) -> Vec<CustomMutationRule> {
        match self {
            Self::Bare(rules) => rules,
            Self::Wrapped { rules } => rules,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct CustomMutationRule {
    name: String,
    description: String,
    explanation: String,
    level: MutationLevel,
    group: String,
    matcher: CustomMutationMatcher,
    edit: CustomMutationEdit,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum CustomMutationMatcher {
    Query { query: String, capture: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum CustomMutationEdit {
    Remove,
    Replace { replacement: String },
}

impl From<CustomMutationRule> for MutationRule {
    fn from(rule: CustomMutationRule) -> Self {
        let matcher = match rule.matcher {
            CustomMutationMatcher::Query { query, capture } => MutationMatcher::Query {
                query: Box::leak(query.into_boxed_str()),
                capture: Box::leak(capture.into_boxed_str()),
            },
        };
        let edit = match rule.edit {
            CustomMutationEdit::Remove => MutationEdit::Remove,
            CustomMutationEdit::Replace { replacement } => MutationEdit::Replace { replacement },
        };

        Self {
            name: rule.name,
            description: rule.description,
            explanation: rule.explanation,
            level: rule.level,
            group: rule.group,
            edit,
            matcher,
        }
    }
}

pub(super) fn load_custom_rules(
    project_root: &Path,
    path: &str,
) -> anyhow::Result<Vec<MutationRule>> {
    let resolved_path = Path::new(path)
        .absolutize_from(project_root)
        .unwrap_or_else(|_| Path::new(path).into())
        .to_path_buf();
    let file_contents = fs::read_to_string(&resolved_path).with_context(|| {
        format!(
            "Failed to read custom mutation rules file '{}'",
            resolved_path.display()
        )
    })?;
    let file: MutationRulesFile = serde_json::from_str(&file_contents).with_context(|| {
        format!(
            "Failed to parse custom mutation rules file '{}' as JSON",
            resolved_path.display()
        )
    })?;

    let custom_rules = file.into_rules();
    let mut seen_names = HashSet::new();
    for rule in &custom_rules {
        if !seen_names.insert(rule.name.clone()) {
            anyhow::bail!(
                "Custom mutation rules file '{}' contains duplicate rule name '{}'",
                resolved_path.display(),
                rule.name
            );
        }
    }

    Ok(custom_rules.into_iter().map(MutationRule::from).collect())
}

pub(super) fn merge_rules(
    built_in_rules: Vec<MutationRule>,
    custom_rules: Vec<MutationRule>,
) -> Vec<MutationRule> {
    let mut merged_rules = built_in_rules;
    let mut built_in_rule_indexes = merged_rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (rule.name.clone(), index))
        .collect::<HashMap<_, _>>();

    for rule in custom_rules {
        if let Some(index) = built_in_rule_indexes.get(&rule.name).copied() {
            merged_rules[index] = rule;
        } else {
            built_in_rule_indexes.insert(rule.name.clone(), merged_rules.len());
            merged_rules.push(rule);
        }
    }

    merged_rules
}
