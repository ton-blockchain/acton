/**
 * @file FunC grammar generated with Tree-sitter
 * @author TON Core
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

function commaSep(rule) {
    return optional(commaSep1(rule))
}

function commaSep1(rule) {
    return seq(rule, repeat(seq(",", rule)))
}

function commaSep2(rule) {
    return seq(rule, repeat1(seq(",", rule)))
}

const FUNC_GRAMMAR = {
    source_file: $ => repeat($._top_level_item),

    // ----------------------------------------------------------
    // top-level declarations

    _top_level_item: $ =>
        choice(
            $.function_declaration,
            $.global_var_declarations,
            $.import_directive,
            $.pragma_directive,
            $.constant_declarations,
            $.empty_statement,
        ),

    import_directive: $ =>
        prec.right(seq("#include", repeat1(" "), field("path", $.string_literal), optional(";"))),

    version_identifier: _ => /(>=|<=|=|>|<|\^)?([0-9]+)(.[0-9]+)?(.[0-9]+)?/,
    pragma_directive: $ =>
        seq(
            "#pragma",
            repeat1(" "),
            choice(
                seq(
                    field("key", choice("version", "not-version")),
                    repeat1(" "),
                    field("value", $.version_identifier),
                ),
                field("key", choice("allow-post-modification", "compute-asm-ltr")),
            ),
        ),

    global_var_declarations: $ =>
        seq("global", field("decls", commaSep1($.global_var_declaration)), ";"),
    global_var_declaration: $ =>
        seq(field("type", optional($._type_hint)), field("name", $.identifier)),

    constant_declarations: $ =>
        seq("const", field("decls", commaSep1($.constant_declaration)), ";"),
    constant_declaration: $ =>
        seq(
            field("type", optional($._type_hint)),
            field("name", $.identifier),
            "=",
            field("value", $.constant_declaration_value),
        ),
    constant_declaration_value: $ => $._expression,

    // ----------------------------------------------------------
    // functions and their body

    function_declaration: $ =>
        seq(
            field("type_parameters", optional($.type_parameters)),
            field("return_type", $._type_hint),
            field("name", $.identifier),
            choice(
                seq(
                    field("parameters", $.parameter_list),
                    field("specifiers", optional($.specifiers_list)),
                    choice(
                        field("body", $.block_statement),
                        field("asm_body", $.asm_function_body),
                    ),
                ),
                seq(
                    field("parameters", $.parameter_list_relaxed),
                    field("specifiers", optional($.specifiers_list)),
                    ";",
                ),
            ),
        ),

    impure: _ => "impure",
    inline: _ => choice("inline", "inline_ref"),
    method_id: $ =>
        seq(
            "method_id",
            optional(seq("(", field("value", choice($.number_literal, $.string_literal)), ")")),
        ),

    specifiers_list: $ =>
        choice(
            seq($.impure, optional($.inline), optional($.method_id)),
            seq($.inline, optional($.method_id)),
            $.method_id,
        ),

    type_parameters: $ => seq("forall", commaSep($.type_parameter), "->"),

    type_parameter: $ => seq(optional("type"), field("name", $.type_identifier)),

    parameter_list: $ => seq("(", commaSep($.parameter_declaration), ")"),

    parameter_list_relaxed: $ =>
        seq(
            "(",
            commaSep(
                choice($.parameter_declaration, field("name", choice($.identifier, $.underscore))),
            ),
            ")",
        ),

    parameter_declaration: $ =>
        seq(
            field("type", $._type_hint),
            optional(field("name", choice($.identifier, $.underscore))),
        ),

    asm_function_body: $ =>
        seq(
            seq(
                "asm",
                optional(
                    seq(
                        "(",
                        repeat($.identifier),
                        optional(seq("->", repeat($.number_literal))),
                        ")",
                    ),
                ),
            ),
            repeat1($.string_literal),
            ";",
        ),

    // ----------------------------------------------------------
    // statements

    _statement: $ =>
        choice(
            $.return_statement,
            $.block_statement,
            $.expression_statement,
            $.empty_statement,
            $.repeat_statement,
            $.if_statement,
            $.do_while_statement,
            $.while_statement,
            $.try_catch_statement,
        ),

    return_statement: $ => seq("return", $._expression, ";"),
    block_statement: $ => seq("{", repeat($._statement), "}"),
    expression_statement: $ => prec.right(seq($._expression, optional(";"))),
    empty_statement: _ => ";",
    repeat_statement: $ =>
        seq("repeat", field("count", $._expression), field("body", $.block_statement)),

    if_statement: $ => seq(choice("if", "ifnot"), $._if_statement_contents),
    _if_statement_contents: $ =>
        seq(
            field("condition", $._expression),
            field("consequent", $.block_statement),
            field(
                "alternative",
                optional(
                    choice(
                        seq("else", $.block_statement),
                        seq(choice("elseif", "elseifnot"), $._if_statement_contents),
                    ),
                ),
            ),
        ),

    do_while_statement: $ =>
        seq("do", field("body", $.block_statement), "until", field("postcondition", $._expression)),
    while_statement: $ =>
        seq("while", field("precondition", $._expression), field("body", $.block_statement)),

    try_catch_statement: $ => seq("try", field("body", $.block_statement), $.catch_clause),
    catch_clause: $ =>
        seq(
            "catch",
            field("catch_expr", optional($._expression)),
            field("catch_body", $.block_statement),
        ),

    // ----------------------------------------------------------
    // expressions

    _expression: $ => $._expr10,

    _expr10: $ =>
        prec(
            10,
            seq(
                $._expr13,
                optional(
                    seq(
                        choice(
                            "=",
                            "+=",
                            "-=",
                            "*=",
                            "/=",
                            "~/=",
                            "^/=",
                            "%=",
                            "~%=",
                            "^%=",
                            "<<=",
                            ">>=",
                            "~>>=",
                            "^>>=",
                            "&=",
                            "|=",
                            "^=",
                        ),
                        $._expr10,
                    ),
                ),
            ),
        ),

    _expr13: $ => prec(13, seq($._expr15, optional(seq("?", $._expression, ":", $._expr13)))),

    _expr15: $ =>
        prec(
            15,
            seq(
                $._expr17,
                optional(seq(choice("==", "<", ">", "<=", ">=", "!=", "<=>"), $._expr17)),
            ),
        ),

    _expr17: $ =>
        prec.left(17, seq($._expr20, repeat(seq(choice("<<", ">>", "~>>", "^>>"), $._expr20)))),

    _expr20: $ =>
        prec.left(
            20,
            seq(optional("-"), $._expr30, repeat(seq(choice("-", "+", "|", "^"), $._expr30))),
        ),

    _expr30: $ =>
        prec.left(
            30,
            seq(
                $._expr75,
                repeat(seq(choice("*", "/", "%", "~/", "^/", "~%", "^%", "/%", "&"), $._expr75)),
            ),
        ),

    _expr75: $ => prec(75, seq(optional("~"), $._expr80)),

    _expr80: $ => prec.left(80, seq($._expr90, repeat($.method_call))),
    method_call: $ =>
        prec.left(
            80,
            seq(
                choice(".", "~"),
                field("method_name", $.identifier),
                field("arguments", $._expr100),
            ),
        ),

    _expr90: $ =>
        prec.left(90, choice($._expr100, $.local_vars_declaration, $.function_application)),
    function_application: $ =>
        prec.left(
            90,
            seq(
                field("callee", $._nontype_expr100),
                field(
                    "arguments",
                    repeat1(choice($.identifier, $.parenthesized_expression, $.tensor_expression)),
                ),
            ),
        ),
    local_vars_declaration: $ => prec.dynamic(90, field("lhs", $._var_declaration_lhs)),

    tuple_vars_declaration: $ =>
        prec(100, seq("[", field("vars", commaSep1($._var_declaration_lhs)), optional(","), "]")),
    tensor_vars_declaration: $ =>
        prec(100, seq("(", field("vars", commaSep1($._var_declaration_lhs)), optional(","), ")")),
    var_declaration: $ => seq(field("type", $._type_hint), field("name", $.identifier)),

    _var_declaration_lhs: $ =>
        choice($.tuple_vars_declaration, $.tensor_vars_declaration, $.var_declaration),

    type_expression: $ =>
        prec(
            101,
            choice(
                $.primitive_type,
                $.type_identifier,
                $.var_type,
                $.parenthesized_type_expression,
                $.tensor_type_expression,
                $.tuple_type_expression,
            ),
        ),
    parenthesized_type_expression: $ => prec(101, seq("(", $.type_expression, ")")),
    tensor_type_expression: $ => prec(101, seq("(", commaSep2($.type_expression), ")")),
    tuple_type_expression: $ => prec(101, seq("[", commaSep1($.type_expression), "]")),

    _nontype_expr100: $ =>
        prec(
            100,
            choice(
                $.parenthesized_expression,
                $.tensor_expression,
                $.local_vars_declaration,
                $.typed_tuple,
                $.identifier,
                $.number_literal,
                $.string_literal,
                $.slice_string_literal,
                $.underscore,
            ),
        ),

    _expr100: $ => prec(100, choice($._nontype_expr100)),

    parenthesized_expression: $ => seq("(", $._expression, ")"),
    tensor_expression: $ =>
        choice(seq("(", ")"), seq("(", field("expressions", commaSep2($._expression)), ")")),
    typed_tuple: $ => seq("[", field("expressions", commaSep($._expression)), "]"),

    // ----------------------------------------------------------
    // type system

    _type_hint: $ => choice($._atomic_type, $.function_type),

    function_type: $ => prec.right(100, seq($._atomic_type, "->", $._type_hint)),

    _atomic_type: $ =>
        choice(
            $.primitive_type,
            $.var_type,
            $.hole_type,
            $.type_identifier,
            $.tensor_type,
            $.tuple_type,
            $._parenthesized_type,
        ),

    _parenthesized_type: $ => seq("(", $._type_hint, ")"),

    primitive_type: $ => choice("int", "cell", "slice", "builder", "cont", "tuple"),
    // constant_type: $ => choice("int", "slice"),

    tensor_type: $ => choice(seq("(", ")"), seq("(", field("types", commaSep2($._type_hint)), ")")),

    tuple_type: $ => seq("[", field("types", commaSep($._type_hint)), "]"),

    var_type: _ => "var",
    hole_type: $ => alias($.underscore, $.hole_type),

    type_identifier: $ => alias($.identifier, $.type_identifier),

    // ----------------------------------------------------------
    // common constructions

    number_literal: $ =>
        choice(
            token(seq(optional("-"), choice(seq("0x", /[0-9a-fA-F]+/), /[0-9]+/))),
            $.number_string_literal,
        ),

    string_literal: _ => /"[^"]*"/,
    number_string_literal: _ => /"[^"]*"[Hhcu]/,
    slice_string_literal: _ => /"[^"]*"[sa]/,

    // actually, FunC identifiers are much more flexible
    identifier: _ => /`[^`]+`|[a-zA-Z0-9_\$%][^\s\+\-\*\/%,\.;\(\)\{\}\[\]=\|\^\~]*/,
    underscore: _ => "_",

    // multiline_comment: $ => seq('{-', repeat(choice(/./, $.multiline_comment)), '-}'),
    // unfortunately getting panic while generating parser with support for nested comments
    comment: $ =>
        token(
            choice(
                seq(";;", /[^\r\n]*/),
                seq("//", /[^\r\n]*/),
                seq("{-", /[^-]*\-+([^-}][^-]*\-+)*/, "}"),
                seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/"),
            ),
        ),
}

export default grammar({
    name: "func",

    conflicts: $ => [
        [$.parameter_list_relaxed, $.type_identifier],
        [$.parameter_list_relaxed, $.hole_type],
        [$.parameter_list_relaxed, $.parameter_list],
        [$.tensor_expression, $.tensor_type],
        [$.typed_tuple, $.tuple_type],
    ],

    extras: $ => [/\s/, $.comment],

    word: $ => $.identifier,

    rules: FUNC_GRAMMAR,
})
