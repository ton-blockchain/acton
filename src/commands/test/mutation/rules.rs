use acton_config::mutation_rules::{
    CustomMutationEdit, CustomMutationMatcher, CustomMutationRule, CustomMutationRulesFile,
};
use acton_config::test::MutationLevel;
use anyhow::Context;
use path_absolutize::Absolutize;
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
            "replace_plus_with_minus",
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
            "replace_minus_with_plus",
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
            "replace_multiply_with_divide",
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
            "replace_divide_with_multiply",
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
            "replace_equal_with_not_equal",
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
            "replace_not_equal_with_equal",
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
            "replace_less_than_with_less_or_equal",
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
            "replace_greater_than_with_greater_or_equal",
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
            "replace_less_or_equal_with_less_than",
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
            "replace_greater_or_equal_with_greater_than",
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
            "replace_true_with_false",
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
            "replace_false_with_true",
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
            "replace_plus_assign_with_minus_assign",
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
            "replace_minus_assign_with_plus_assign",
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
            "replace_logical_and_with_logical_or",
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
            "replace_logical_or_with_logical_and",
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
            "replace_bitwise_and_with_bitwise_or",
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
            "replace_bitwise_or_with_bitwise_and",
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
            "replace_bitwise_and_with_bitwise_xor",
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
            "replace_bitwise_or_with_bitwise_xor",
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
            "replace_bitwise_xor_with_bitwise_and",
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
            "replace_bitwise_xor_with_bitwise_or",
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
            "replace_bitwise_and_assign_with_bitwise_or_assign",
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
            "replace_bitwise_and_assign_with_bitwise_xor_assign",
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
            "replace_bitwise_or_assign_with_bitwise_and_assign",
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
            "replace_bitwise_or_assign_with_bitwise_xor_assign",
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
            "replace_bitwise_xor_assign_with_bitwise_and_assign",
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
            "replace_bitwise_xor_assign_with_bitwise_or_assign",
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
            "replace_left_shift_assign_with_right_shift_assign",
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
            "replace_right_shift_assign_with_left_shift_assign",
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
            "replace_left_shift_with_right_shift",
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
            "replace_right_shift_with_left_shift",
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
            "replace_multiply_assign_with_divide_assign",
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
            "replace_divide_assign_with_multiply_assign",
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
            "replace_multiply_assign_with_modulo_assign",
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
            "replace_modulo_assign_with_multiply_assign",
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
            "replace_if_condition_with_true",
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
            "replace_if_condition_with_false",
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
            "replace_while_condition_with_false",
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
    let file: CustomMutationRulesFile =
        serde_json::from_str(&file_contents).with_context(|| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test::mutation::{
        MutationSpan, collect_mutations, remove_span_from_source, replace_span_in_source,
    };
    use tree_sitter::Parser;

    fn parse_fixture(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&tolk_syntax::language())
            .expect("tolk grammar should load");
        let tree = parser.parse(source, None).expect("fixture should parse");
        assert!(
            !tree.root_node().has_error(),
            "fixture should not contain syntax errors"
        );
        tree
    }

    fn apply_candidate(
        source: &str,
        candidate: &crate::commands::test::mutation::MutationCandidate<'_>,
    ) -> String {
        let span = MutationSpan::from_node(&candidate.node);
        match &candidate.rule.edit {
            MutationEdit::Remove => remove_span_from_source(source, span),
            MutationEdit::Replace { replacement } => {
                replace_span_in_source(source, span, replacement)
            }
        }
    }

    fn find_rule(rule_name: &str) -> MutationRule {
        rules()
            .into_iter()
            .find(|rule| rule.name == rule_name)
            .unwrap_or_else(|| panic!("rule '{rule_name}' should exist"))
    }

    fn assert_rule_mutates_to(rule_name: &str, before: &str, after: &str) {
        let tree = parse_fixture(before);
        let rule = find_rule(rule_name);

        let candidates = collect_mutations(tree.root_node(), before, std::slice::from_ref(&rule))
            .unwrap_or_else(|err| panic!("rule '{rule_name}' should collect candidates: {err}"));

        assert_eq!(
            candidates.len(),
            1,
            "rule '{rule_name}' should match exactly once"
        );

        let mutated = apply_candidate(before, &candidates[0]);
        assert_eq!(
            mutated, after,
            "rule '{rule_name}' produced unexpected mutated source"
        );

        parse_fixture(&mutated);
    }

    #[test]
    fn remove_assert_changes_source() {
        let before = "\
fun f(a: int, b: int) {
    val keep = 1;
    assert (a == b) throw 1;
    val result = keep;
}
";
        let after = "\
fun f(a: int, b: int) {
    val keep = 1;
    val result = keep;
}
";
        assert_rule_mutates_to("remove_assert", before, after);
    }

    #[test]
    fn remove_throw_changes_source() {
        let before = "\
fun f() {
    if (true) {
        val keep = 1;
        throw 42;
    }
}
";
        let after = "\
fun f() {
    if (true) {
        val keep = 1;
    }
}
";
        assert_rule_mutates_to("remove_throw", before, after);
    }

    #[test]
    fn remove_storage_save_call_changes_source() {
        let before = "\
fun f() {
    val keep = 1;
    storage.save();
    val result = keep;
}
";
        let after = "\
fun f() {
    val keep = 1;
    val result = keep;
}
";
        assert_rule_mutates_to("remove_storage_save_call", before, after);
    }

    #[test]
    fn remove_set_data_call_changes_source() {
        let before = "\
fun f() {
    val keep = 1;
    contract.setData(createEmptyCell());
    val result = keep;
}
";
        let after = "\
fun f() {
    val keep = 1;
    val result = keep;
}
";
        assert_rule_mutates_to("remove_set_data_call", before, after);
    }

    #[test]
    fn remove_accept_external_message_changes_source() {
        let before = "\
fun f() {
    val keep = 1;
    acceptExternalMessage();
    val result = keep;
}
";
        let after = "\
fun f() {
    val keep = 1;
    val result = keep;
}
";
        assert_rule_mutates_to("remove_accept_external_message", before, after);
    }

    #[test]
    fn remove_commit_contract_data_and_actions_changes_source() {
        let before = "\
fun f() {
    val keep = 1;
    commitContractDataAndActions();
    val result = keep;
}
";
        let after = "\
fun f() {
    val keep = 1;
    val result = keep;
}
";
        assert_rule_mutates_to("remove_commit_contract_data_and_actions", before, after);
    }

    #[test]
    fn remove_set_code_postponed_changes_source() {
        let before = "\
fun f() {
    val keep = 1;
    contract.setCodePostponed(createEmptyCell());
    val result = keep;
}
";
        let after = "\
fun f() {
    val keep = 1;
    val result = keep;
}
";
        assert_rule_mutates_to("remove_set_code_postponed", before, after);
    }

    #[test]
    fn replace_plus_with_minus_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a + b; }";
        let after = r"fun f(a: int, b: int) { val result = a - b; }";
        assert_rule_mutates_to("replace_plus_with_minus", before, after);
    }

    #[test]
    fn replace_minus_with_plus_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a - b; }";
        let after = r"fun f(a: int, b: int) { val result = a + b; }";
        assert_rule_mutates_to("replace_minus_with_plus", before, after);
    }

    #[test]
    fn replace_multiply_with_divide_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a * b; }";
        let after = r"fun f(a: int, b: int) { val result = a / b; }";
        assert_rule_mutates_to("replace_multiply_with_divide", before, after);
    }

    #[test]
    fn replace_divide_with_multiply_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a / b; }";
        let after = r"fun f(a: int, b: int) { val result = a * b; }";
        assert_rule_mutates_to("replace_divide_with_multiply", before, after);
    }

    #[test]
    fn replace_equal_with_not_equal_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a == b; }";
        let after = r"fun f(a: int, b: int) { val result = a != b; }";
        assert_rule_mutates_to("replace_equal_with_not_equal", before, after);
    }

    #[test]
    fn replace_not_equal_with_equal_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a != b; }";
        let after = r"fun f(a: int, b: int) { val result = a == b; }";
        assert_rule_mutates_to("replace_not_equal_with_equal", before, after);
    }

    #[test]
    fn replace_less_than_with_less_or_equal_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a < b; }";
        let after = r"fun f(a: int, b: int) { val result = a <= b; }";
        assert_rule_mutates_to("replace_less_than_with_less_or_equal", before, after);
    }

    #[test]
    fn replace_greater_than_with_greater_or_equal_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a > b; }";
        let after = r"fun f(a: int, b: int) { val result = a >= b; }";
        assert_rule_mutates_to("replace_greater_than_with_greater_or_equal", before, after);
    }

    #[test]
    fn replace_less_or_equal_with_less_than_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a <= b; }";
        let after = r"fun f(a: int, b: int) { val result = a < b; }";
        assert_rule_mutates_to("replace_less_or_equal_with_less_than", before, after);
    }

    #[test]
    fn replace_greater_or_equal_with_greater_than_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a >= b; }";
        let after = r"fun f(a: int, b: int) { val result = a > b; }";
        assert_rule_mutates_to("replace_greater_or_equal_with_greater_than", before, after);
    }

    #[test]
    fn replace_true_with_false_changes_source() {
        let before = r"fun f() { val result = true; }";
        let after = r"fun f() { val result = false; }";
        assert_rule_mutates_to("replace_true_with_false", before, after);
    }

    #[test]
    fn replace_false_with_true_changes_source() {
        let before = r"fun f() { val result = false; }";
        let after = r"fun f() { val result = true; }";
        assert_rule_mutates_to("replace_false_with_true", before, after);
    }

    #[test]
    fn replace_plus_assign_with_minus_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value += b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value -= b; }";
        assert_rule_mutates_to("replace_plus_assign_with_minus_assign", before, after);
    }

    #[test]
    fn replace_minus_assign_with_plus_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value -= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value += b; }";
        assert_rule_mutates_to("replace_minus_assign_with_plus_assign", before, after);
    }

    #[test]
    fn replace_logical_and_with_logical_or_changes_source() {
        let before = r"fun f(left: bool, right: bool) { val result = left && right; }";
        let after = r"fun f(left: bool, right: bool) { val result = left || right; }";
        assert_rule_mutates_to("replace_logical_and_with_logical_or", before, after);
    }

    #[test]
    fn replace_logical_or_with_logical_and_changes_source() {
        let before = r"fun f(left: bool, right: bool) { val result = left || right; }";
        let after = r"fun f(left: bool, right: bool) { val result = left && right; }";
        assert_rule_mutates_to("replace_logical_or_with_logical_and", before, after);
    }

    #[test]
    fn replace_bitwise_and_with_bitwise_or_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a & b; }";
        let after = r"fun f(a: int, b: int) { val result = a | b; }";
        assert_rule_mutates_to("replace_bitwise_and_with_bitwise_or", before, after);
    }

    #[test]
    fn replace_bitwise_or_with_bitwise_and_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a | b; }";
        let after = r"fun f(a: int, b: int) { val result = a & b; }";
        assert_rule_mutates_to("replace_bitwise_or_with_bitwise_and", before, after);
    }

    #[test]
    fn replace_bitwise_and_with_bitwise_xor_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a & b; }";
        let after = r"fun f(a: int, b: int) { val result = a ^ b; }";
        assert_rule_mutates_to("replace_bitwise_and_with_bitwise_xor", before, after);
    }

    #[test]
    fn replace_bitwise_or_with_bitwise_xor_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a | b; }";
        let after = r"fun f(a: int, b: int) { val result = a ^ b; }";
        assert_rule_mutates_to("replace_bitwise_or_with_bitwise_xor", before, after);
    }

    #[test]
    fn replace_bitwise_xor_with_bitwise_and_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a ^ b; }";
        let after = r"fun f(a: int, b: int) { val result = a & b; }";
        assert_rule_mutates_to("replace_bitwise_xor_with_bitwise_and", before, after);
    }

    #[test]
    fn replace_bitwise_xor_with_bitwise_or_changes_source() {
        let before = r"fun f(a: int, b: int) { val result = a ^ b; }";
        let after = r"fun f(a: int, b: int) { val result = a | b; }";
        assert_rule_mutates_to("replace_bitwise_xor_with_bitwise_or", before, after);
    }

    #[test]
    fn replace_bitwise_and_assign_with_bitwise_or_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value &= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value |= b; }";
        assert_rule_mutates_to(
            "replace_bitwise_and_assign_with_bitwise_or_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_bitwise_and_assign_with_bitwise_xor_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value &= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value ^= b; }";
        assert_rule_mutates_to(
            "replace_bitwise_and_assign_with_bitwise_xor_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_bitwise_or_assign_with_bitwise_and_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value |= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value &= b; }";
        assert_rule_mutates_to(
            "replace_bitwise_or_assign_with_bitwise_and_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_bitwise_or_assign_with_bitwise_xor_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value |= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value ^= b; }";
        assert_rule_mutates_to(
            "replace_bitwise_or_assign_with_bitwise_xor_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_bitwise_xor_assign_with_bitwise_and_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value ^= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value &= b; }";
        assert_rule_mutates_to(
            "replace_bitwise_xor_assign_with_bitwise_and_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_bitwise_xor_assign_with_bitwise_or_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value ^= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value |= b; }";
        assert_rule_mutates_to(
            "replace_bitwise_xor_assign_with_bitwise_or_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_left_shift_assign_with_right_shift_assign_changes_source() {
        let before = r"fun f(value: int) { var result = value; result <<= 1; }";
        let after = r"fun f(value: int) { var result = value; result >>= 1; }";
        assert_rule_mutates_to(
            "replace_left_shift_assign_with_right_shift_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_right_shift_assign_with_left_shift_assign_changes_source() {
        let before = r"fun f(value: int) { var result = value; result >>= 1; }";
        let after = r"fun f(value: int) { var result = value; result <<= 1; }";
        assert_rule_mutates_to(
            "replace_right_shift_assign_with_left_shift_assign",
            before,
            after,
        );
    }

    #[test]
    fn replace_left_shift_with_right_shift_changes_source() {
        let before = r"fun f(value: int) { val result = value << 1; }";
        let after = r"fun f(value: int) { val result = value >> 1; }";
        assert_rule_mutates_to("replace_left_shift_with_right_shift", before, after);
    }

    #[test]
    fn replace_right_shift_with_left_shift_changes_source() {
        let before = r"fun f(value: int) { val result = value >> 1; }";
        let after = r"fun f(value: int) { val result = value << 1; }";
        assert_rule_mutates_to("replace_right_shift_with_left_shift", before, after);
    }

    #[test]
    fn remove_logical_not_changes_source() {
        let before = r"fun f(flag: bool) { val result = !flag; }";
        let after = r"fun f(flag: bool) { val result = flag; }";
        assert_rule_mutates_to("remove_logical_not", before, after);
    }

    #[test]
    fn remove_bitwise_not_changes_source() {
        let before = r"fun f(value: int) { val result = ~value; }";
        let after = r"fun f(value: int) { val result = value; }";
        assert_rule_mutates_to("remove_bitwise_not", before, after);
    }

    #[test]
    fn replace_multiply_assign_with_divide_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value *= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value /= b; }";
        assert_rule_mutates_to("replace_multiply_assign_with_divide_assign", before, after);
    }

    #[test]
    fn replace_divide_assign_with_multiply_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value /= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value *= b; }";
        assert_rule_mutates_to("replace_divide_assign_with_multiply_assign", before, after);
    }

    #[test]
    fn replace_multiply_assign_with_modulo_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value *= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value %= b; }";
        assert_rule_mutates_to("replace_multiply_assign_with_modulo_assign", before, after);
    }

    #[test]
    fn replace_modulo_assign_with_multiply_assign_changes_source() {
        let before = r"fun f(a: int, b: int) { var value = a; value %= b; }";
        let after = r"fun f(a: int, b: int) { var value = a; value *= b; }";
        assert_rule_mutates_to("replace_modulo_assign_with_multiply_assign", before, after);
    }

    #[test]
    fn remove_unary_minus_changes_source() {
        let before = r"fun f(value: int) { val result = -value; }";
        let after = r"fun f(value: int) { val result = value; }";
        assert_rule_mutates_to("remove_unary_minus", before, after);
    }

    #[test]
    fn replace_if_condition_with_true_changes_source() {
        let before = r"fun f(flag: bool) { if (flag) { throw 1; } }";
        let after = r"fun f(flag: bool) { if (true) { throw 1; } }";
        assert_rule_mutates_to("replace_if_condition_with_true", before, after);
    }

    #[test]
    fn replace_if_condition_with_false_changes_source() {
        let before = r"fun f(flag: bool) { if (flag) { throw 1; } }";
        let after = r"fun f(flag: bool) { if (false) { throw 1; } }";
        assert_rule_mutates_to("replace_if_condition_with_false", before, after);
    }

    #[test]
    fn replace_while_condition_with_false_changes_source() {
        let before = r"fun f(flag: bool) { var keep = flag; while (keep) { keep = false; } }";
        let after = r"fun f(flag: bool) { var keep = flag; while (false) { keep = false; } }";
        assert_rule_mutates_to("replace_while_condition_with_false", before, after);
    }

    #[test]
    fn remove_throw_does_not_target_throw_inside_assert() {
        let before = "\
fun f(a: int, b: int) {
    assert (a == b) throw 1;
    if (true) {
        val keep = 1;
        throw 42;
    }
}
";
        let after = "\
fun f(a: int, b: int) {
    assert (a == b) throw 1;
    if (true) {
        val keep = 1;
    }
}
";
        let tree = parse_fixture(before);
        let rule = find_rule("remove_throw");

        let candidates = collect_mutations(tree.root_node(), before, std::slice::from_ref(&rule))
            .expect("remove_throw should collect candidates");

        assert_eq!(
            candidates.len(),
            1,
            "fixture should expose one standalone throw"
        );

        let mutated = apply_candidate(before, &candidates[0]);
        assert_eq!(mutated, after);
    }
}
