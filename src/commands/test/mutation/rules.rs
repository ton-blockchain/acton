use owo_colors::OwoColorize;
use tree_sitter::Node;

pub(crate) fn rules() -> Vec<MutationRule> {
    vec![
        MutationRule::remove(
            "remove_assert",
            "Remove assert statements",
            "This assertion is not covered by tests. This could lead to security vulnerabilities if the condition is not enforced.",
            MutationLevel::Critical,
            MutationMatcher::Query {
                query: r#"(assert_statement) @assert"#,
                capture: "assert",
            },
        ),
        MutationRule::remove(
            "remove_throw",
            "Remove throw keyword",
            "This exception path is not covered by tests. Missing error handling might leave the contract in an inconsistent state.",
            MutationLevel::Critical,
            MutationMatcher::Callback {
                predicate: |node, _| -> anyhow::Result<bool> {
                    if node.kind() != "throw" {
                        return Ok(false);
                    }
                    let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
                    Ok(parent_kind != "assert_statement")
                },
            },
        ),
        MutationRule::replace(
            "flip_plus",
            "Replace + with -",
            "This arithmetic operation is not fully covered by tests. Changing + to - did not cause any tests to fail.",
            MutationLevel::Major,
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
            MutationMatcher::Query {
                query: r#"(set_assignment operator_name: "-=" @op)"#,
                capture: "op",
            },
            "+=",
        ),
    ]
}

#[derive(Clone)]
pub(crate) enum MutationEdit {
    Remove,
    Replace { replacement: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum MutationLevel {
    Critical,
    Major,
    Minor,
}

impl MutationLevel {
    pub(crate) fn label(&self) -> &'static str {
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

pub(crate) type NodePredicate = for<'a> fn(Node<'a>, &str) -> anyhow::Result<bool>;

#[derive(Clone)]
pub(crate) enum MutationMatcher {
    Query {
        query: &'static str,
        capture: &'static str,
    },
    Callback {
        predicate: NodePredicate,
    },
}

#[derive(Clone)]
pub(crate) struct MutationRule {
    pub(crate) name: &'static str,
    pub description: &'static str,
    pub explanation: &'static str,
    pub level: MutationLevel,
    pub edit: MutationEdit,
    pub matcher: MutationMatcher,
}

impl MutationRule {
    fn remove(
        name: &'static str,
        description: &'static str,
        explanation: &'static str,
        level: MutationLevel,
        matcher: MutationMatcher,
    ) -> Self {
        Self {
            name,
            description,
            explanation,
            level,
            edit: MutationEdit::Remove,
            matcher,
        }
    }

    fn replace(
        name: &'static str,
        description: &'static str,
        explanation: &'static str,
        level: MutationLevel,
        matcher: MutationMatcher,
        replacement: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            explanation,
            level,
            edit: MutationEdit::Replace { replacement },
            matcher,
        }
    }
}
