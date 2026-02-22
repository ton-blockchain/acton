use acton_config::color::OwoColorize;
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
    ]
}

#[derive(Clone)]
pub(super) enum MutationEdit {
    Remove,
    Replace { replacement: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
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

    pub(crate) fn colorize(&self, text: &str) -> String {
        match self {
            MutationLevel::Critical => text.red().bold().to_string(),
            MutationLevel::Major => text.yellow().bold().to_string(),
            MutationLevel::Minor => text.dimmed().to_string(),
        }
    }
}

pub(super) type NodePredicate = for<'a> fn(Node<'a>, &str) -> anyhow::Result<bool>;

#[derive(Clone)]
pub(super) enum MutationMatcher {
    Query {
        query: &'static str,
        capture: &'static str,
    },
    Callback {
        predicate: NodePredicate,
    },
}

#[derive(Clone)]
pub(super) struct MutationRule {
    pub name: &'static str,
    pub description: &'static str,
    pub explanation: &'static str,
    pub level: MutationLevel,
    pub group: &'static str,
    pub edit: MutationEdit,
    pub matcher: MutationMatcher,
}

impl MutationRule {
    const fn remove(
        name: &'static str,
        description: &'static str,
        explanation: &'static str,
        level: MutationLevel,
        group: &'static str,
        matcher: MutationMatcher,
    ) -> Self {
        Self {
            name,
            description,
            explanation,
            level,
            group,
            edit: MutationEdit::Remove,
            matcher,
        }
    }

    const fn replace(
        name: &'static str,
        description: &'static str,
        explanation: &'static str,
        level: MutationLevel,
        group: &'static str,
        matcher: MutationMatcher,
        replacement: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            explanation,
            level,
            group,
            edit: MutationEdit::Replace { replacement },
            matcher,
        }
    }
}
