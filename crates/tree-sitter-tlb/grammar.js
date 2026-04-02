/**
 * @file TL-B grammar for tree-sitter
 * @author TON Core
 * @author TON Studio
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
    name: "tlb",

    extras: $ => [/\s/, $.comment],

    conflicts: _ => [],

    precedences: _ => [
        [
            "unary",
            "conditional",
            "array",
            "binary_comparison",
            "binary_addition",
            "binary_multiplication",
        ],
    ],

    rules: {
        program: $ => repeat($.declaration),

        declaration: $ =>
            seq(
                field("constructor", $.constructor_),
                repeat($.field),
                "=",
                field("combinator", $.combinator),
                ";",
            ),

        constructor_: $ =>
            prec.right(
                seq(
                    optional("!"),
                    field("name", choice(alias("_", $.identifier), $.identifier)),
                    field("tag", optional($.constructor_tag)),
                ),
            ),

        constructor_tag: $ =>
            choice(
                prec.right(seq("$", choice(alias("_", $.identifier), $.binary_number))),
                prec.right(seq("#", choice(alias("_", $.identifier), $.hex))),
            ),

        field: $ =>
            choice(
                $.field_builtin,
                $.field_curly_expr,
                $.field_anonymous,
                $.field_named,
                $.field_expr,
            ),

        field_builtin: $ =>
            seq("{", field("name", $.identifier), ":", field("field", $.builtin_field), "}"),

        field_curly_expr: $ => seq("{", field("expr", optional($.curly_expression)), "}"),

        field_anonymous: $ => choice($.field_anon_ref, $.field_named_anon_ref),

        field_named: $ => seq(field("name", $.identifier), ":", field("expr", $.cond_expr)),

        field_expr: $ => $.cond_expr,

        identifier: _ => /[a-zA-Z_][a-zA-Z0-9_]*/,
        _type_identifier: _ => /[a-zA-Z_][a-zA-Z0-9_]*/,

        number: _ => /[0-9]+/,
        binary_number: _ => /[01]+/,
        hex: _ => /[0-9a-fA-F]+/,

        builtin_field: _ => choice("#", "Type"),

        combinator: $ =>
            seq(
                field("name", alias($._type_identifier, $.type_identifier)),
                field("params", repeat($.type_parameter)),
            ),

        type_parameter: $ => $.simple_expr,

        simple_expr: $ =>
            prec.left(2, choice($.negate_expr, $.binary_expression, $.ref_expr, $.parens_expr)),

        negate_expr: $ =>
            prec.right("unary", seq(field("operator", "~"), field("operand", $.simple_expr))),

        binary_expression: $ =>
            choice(
                ...[
                    ["<=", "binary_comparison"],
                    [">=", "binary_comparison"],
                    ["!=", "binary_comparison"],
                    ["=", "binary_comparison"],
                    ["<", "binary_comparison"],
                    [">", "binary_comparison"],

                    ["+", "binary_addition"],

                    ["*", "binary_multiplication"],
                ].map(([operator, precedence]) =>
                    prec.left(
                        precedence,
                        seq(
                            field("left", $.simple_expr),
                            field("operator", operator),
                            field("right", choice($.simple_expr, $.bit_size_expr)),
                        ),
                    ),
                ),
            ),

        math_expr: $ => $.binary_expression,

        ref_expr: $ => prec.left(1, choice($.ref_inner, $.parens_expr)),

        ref_inner: $ =>
            prec.left("array", choice(alias($._type_identifier, $.type_identifier), $.number)),

        parens_expr: $ => seq("(", $.simple_expr, ")"),

        cond_expr: $ =>
            choice($.cond_dot_and_question_expr, $.cond_question_expr, $.cond_type_expr),

        cond_dot_and_question_expr: $ =>
            seq(choice($.cond_dotted, $.parens_cond_dotted), "?", $.type_expr),

        cond_dotted: $ => seq($.type_expr, ".", $.number),

        parens_cond_dotted: $ => seq("(", $.cond_dotted, ")"),

        cond_question_expr: $ => seq($.type_expr, "?", $.type_expr),

        cond_type_expr: $ => prec.left("conditional", $.type_expr),

        type_expr: $ =>
            prec.left(
                1,
                choice(
                    $.cell_ref_expr,
                    $.builtin_expr,
                    $.combinator_expr,
                    $.simple_expr,
                    $.array_type,
                    $.array_multiplier,
                    $.bit_size_expr,
                    $.parens_type_expr,
                ),
            ),

        cell_ref_expr: $ => seq("^", field("expr", choice($.cell_ref_inner, $.parens_cell_ref))),

        cell_ref_inner: $ =>
            choice($.combinator_expr, alias($._type_identifier, $.type_identifier)),

        parens_cell_ref: $ => seq("(", $.cell_ref_inner, ")"),

        builtin_expr: $ => choice($.builtin_one_arg, $.builtin_zero_args),

        builtin_one_arg: $ =>
            seq("(", field("operator", choice("#<=", "#<")), field("expr", $.ref_expr), ")"),

        builtin_zero_args: _ => "#",

        combinator_expr: $ =>
            seq(
                "(",
                field("name", alias($._type_identifier, $.type_identifier)),
                field("params", repeat1($.type_expr)),
                ")",
            ),

        parens_type_expr: $ => seq("(", $.type_expr, ")"),

        compare_expr: $ => choice($.binary_expression, $.parens_compare_expr),

        parens_compare_expr: $ => seq("(", $.compare_expr, ")"),

        curly_expression: $ => choice($.compare_expr, $.identifier),

        field_anon_ref: $ => seq(optional("^"), "[", repeat($.field), "]"),

        field_named_anon_ref: $ => seq($.identifier, ":", $.field_anon_ref),

        array_type: $ =>
            prec.right(
                "array",
                seq(
                    "[",
                    field(
                        "element_type",
                        choice(alias($._type_identifier, $.type_identifier), $.type_expr),
                    ),
                    "]",
                ),
            ),

        array_multiplier: $ =>
            prec.left(
                "binary_multiplication",
                seq(field("size", $.simple_expr), "*", field("type", $.array_type)),
            ),

        bit_size_expr: $ =>
            prec.right(
                "binary_multiplication",
                seq("##", field("size", choice($.number, $.parens_expr))),
            ),

        comment: _ => choice(seq("//", /.*/), seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/")),
    },
})
