/**
 * @file Tolk grammar for tree-sitter
 * @author TON Core
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

function commaSep(rule) {
    return optional(commaSep1(rule))
}

function pipeSep1(rule) {
    return seq(rule, repeat1(seq("|", rule)))
}

function commaSep1(rule) {
    return seq(rule, repeat(seq(",", rule)))
}

function commaSep2(rule) {
    return seq(rule, repeat1(seq(",", rule)))
}

const TOLK_GRAMMAR = {
    source_file: $ => repeat($._top_level_declaration),

    // ----------------------------------------------------------
    // top-level declarations

    _top_level_declaration: $ =>
        choice(
            $.tolk_required_version,
            $.import_directive,
            $.contract_declaration,
            $.global_var_declaration,
            $.constant_declaration,
            $.type_alias_declaration,
            $.struct_declaration,
            $.enum_declaration,
            $.function_declaration,
            $.method_declaration,
            $.get_method_declaration,
            $.empty_statement,
        ),

    tolk_required_version: $ => seq("tolk", field("value", $.version_value)),
    version_value: $ => /(\d+)(.\d+)?(.\d+)?/,

    import_directive: $ => seq("import", field("path", $.string_literal)),

    contract_declaration: $ =>
        seq("contract", field("name", $.identifier), field("body", $.contract_body)),

    contract_body: $ => seq("{", repeat(seq($.contract_field, optional(","))), "}"),

    contract_field: $ =>
        seq(field("name", $.identifier), ":", field("value", choice($._type_hint, $._expression))),

    global_var_declaration: $ =>
        prec.right(
            seq(
                optional(field("annotations", $.annotation_list)),
                "global",
                field("name", $.identifier),
                ":",
                field("type", $._type_hint),
                optional(";"),
            ),
        ),

    constant_declaration: $ =>
        prec.right(
            seq(
                optional(field("annotations", $.annotation_list)),
                "const",
                field("name", $.identifier),
                optional(seq(":", field("type", $._type_hint))),
                "=",
                field("value", $._expression),
                optional(";"),
            ),
        ),

    type_alias_declaration: $ =>
        prec.right(
            seq(
                optional(field("annotations", $.annotation_list)),
                "type",
                field("name", $.identifier),
                optional(field("type_parameters", $.type_parameters)),
                "=",
                optional("|"),
                field("underlying_type", choice($._type_hint, $.builtin_specifier)),
                optional(";"),
            ),
        ),

    struct_declaration: $ =>
        seq(
            optional(field("annotations", $.annotation_list)),
            "struct",
            optional(seq("(", field("pack_prefix", $.number_literal), ")")),
            field("name", $.identifier),
            optional(field("type_parameters", $.type_parameters)),
            optional(field("body", $.struct_body)),
        ),
    struct_body: $ =>
        seq(
            "{",
            optional(
                seq(
                    $.struct_field_declaration,
                    repeat(seq(optional(choice(",", ";")), $.struct_field_declaration)),
                ),
            ),
            optional(choice(",", ";")),
            "}",
        ),
    struct_field_declaration: $ =>
        seq(
            field("modifiers", optional($.struct_field_modifiers)),
            field("name", $.identifier),
            ":",
            field("type", $._type_hint),
            optional(seq("=", field("default", $._expression))),
        ),

    struct_field_modifiers: $ => repeat1(choice("readonly", "private")),

    enum_declaration: $ =>
        seq(
            optional(field("annotations", $.annotation_list)),
            "enum",
            field("name", $.identifier),
            optional(seq(":", field("backed_type", $._type_hint))),
            optional(field("body", $.enum_body)),
        ),
    enum_body: $ =>
        seq(
            "{",
            optional(
                seq(
                    $.enum_member_declaration,
                    repeat(seq(optional(","), $.enum_member_declaration)),
                ),
            ),
            optional(","),
            "}",
        ),
    enum_member_declaration: $ =>
        seq(field("name", $.identifier), optional(seq("=", field("default", $._expression)))),

    // ----------------------------------------------------------
    // functions and their body

    _function_body: $ =>
        choice(
            field("body", $.block_statement),
            field("asm_body", $.asm_body),
            field("builtin_specifier", $.builtin_specifier),
        ),

    function_declaration: $ =>
        seq(
            optional(field("annotations", $.annotation_list)),
            "fun",
            field("name", $.identifier),
            optional(field("type_parameters", $.type_parameters)),
            optional(field("parameters", $.parameter_list)),
            optional(seq(":", field("return_type", optional($._type_hint)))),
            optional($._function_body),
        ),

    method_receiver: $ => seq(field("receiver_type", $._type_hint), "."),
    method_declaration: $ =>
        seq(
            optional(field("annotations", $.annotation_list)),
            "fun",
            field("receiver", $.method_receiver),
            field("name", $.identifier),
            optional(field("type_parameters", $.type_parameters)),
            optional(field("parameters", $.parameter_list)),
            optional(seq(":", field("return_type", optional($._type_hint)))),
            optional($._function_body),
        ),
    get_method_declaration: $ =>
        seq(
            optional(field("annotations", $.annotation_list)),
            "get",
            optional("fun"),
            field("name", $.identifier),
            optional(field("parameters", $.parameter_list)),
            optional(seq(":", field("return_type", optional($._type_hint)))),
            optional(field("body", $._function_body)),
        ),

    annotation_list: $ => repeat1($.annotation),
    annotation_name: _ => /[a-zA-Z0-9_.]+/,
    annotation: $ =>
        seq(
            "@",
            optional(field("name", $.annotation_name)),
            optional(field("arguments", $.annotation_arguments)),
        ),

    annotation_arguments: $ => seq("(", commaSep($._expression), optional(","), ")"),

    type_parameters: $ => seq("<", commaSep($.type_parameter), optional(","), ">"),
    type_parameter: $ =>
        seq(field("name", $.identifier), optional(seq("=", field("default", $._type_hint)))),

    parameter_list: $ => seq("(", commaSep($.parameter_declaration), optional(","), ")"),
    parameter_declaration: $ =>
        seq(
            field("mutate", optional("mutate")),
            field("name", $.identifier),
            optional(seq(":", field("type", $._type_hint))),
            optional(seq("=", field("default", $._expression))),
        ),

    asm_body: $ =>
        prec.right(
            seq(
                "asm",
                optional(field("rearrange", $.asm_body_rearrange)),
                repeat1($.string_literal),
                optional(";"),
            ),
        ),

    asm_body_rearrange: $ =>
        seq(
            "(",
            optional(field("params", $.asm_body_rearrange_params)),
            optional(field("return", $.asm_body_rearrange_return)),
            ")",
        ),

    asm_body_rearrange_params: $ => repeat1($.identifier),
    asm_body_rearrange_return: $ => seq("->", repeat($.number_literal)),

    builtin_specifier: $ => "builtin",

    // ----------------------------------------------------------
    // statements

    _statement_ending_with_brace: $ =>
        choice(
            $.block_statement,
            $.if_statement,
            $.while_statement,
            $.repeat_statement,
            $.try_catch_statement,
            $.empty_statement,
            $.match_statement,
        ),
    _statement_require_semicolon_unless_last: $ =>
        choice(
            // $.local_vars_declaration,
            $.return_statement,
            $.do_while_statement,
            $.break_statement,
            $.continue_statement,
            $.throw_statement,
            $.assert_statement,
            $.expression_statement,
        ),
    _statement: $ =>
        choice(
            $._statement_ending_with_brace,
            prec.right(seq($._statement_require_semicolon_unless_last, optional(";"))),
        ),

    // local_vars_declaration: $ =>
    //     seq(
    //         field("kind", choice("var", "val")),
    //         field("lhs", $.var_declaration_lhs),
    //         optional(seq("=", field("assigned_val", $._expression))),
    //     ),

    tuple_vars_declaration: $ =>
        seq("[", field("vars", commaSep1($._var_declaration_lhs)), optional(","), "]"),
    tensor_vars_declaration: $ =>
        seq("(", field("vars", commaSep1($._var_declaration_lhs)), optional(","), ")"),
    var_declaration: $ =>
        prec.right(
            seq(
                field("name", $.identifier),
                optional(choice(seq(":", field("type", $._type_hint)), field("redef", "redef"))),
            ),
        ),
    _var_declaration_lhs: $ =>
        choice($.tuple_vars_declaration, $.tensor_vars_declaration, $.var_declaration),

    var_declaration_lhs: $ => seq(field("kind", choice("var", "val")), $._var_declaration_lhs),

    block_statement: $ =>
        prec.dynamic(
            100,
            seq(
                "{",
                repeat($._statement),
                optional($._statement_require_semicolon_unless_last),
                "}",
            ),
        ),

    return_statement: $ => prec.right(seq("return", optional(field("body", $._expression)))),

    repeat_statement: $ =>
        seq("repeat", "(", field("count", $._expression), ")", field("body", $.block_statement)),

    if_statement: $ =>
        prec.right(
            seq(
                "if",
                "(",
                field("condition", $._expression),
                ")",
                optional(field("body", $.block_statement)),
                optional(
                    choice(
                        seq("else", field("alternative", $.if_statement)),
                        seq("else", field("alternative", $.block_statement)),
                    ),
                ),
            ),
        ),

    do_while_statement: $ =>
        seq(
            "do",
            field("body", $.block_statement),
            "while",
            "(",
            field("condition", $._expression),
            ")",
        ),

    while_statement: $ =>
        seq("while", "(", field("condition", $._expression), ")", field("body", $.block_statement)),

    break_statement: $ => "break",
    continue_statement: $ => "continue",

    throw_statement: $ =>
        seq(
            "throw",
            field("excNo", $._expression), // excNo, (excNo), (excNo, arg); but (1,2,3) will be also acceptable
        ),

    assert_statement: $ =>
        seq(
            "assert",
            choice(
                seq(
                    "(",
                    field("condition", $._expression),
                    ")",
                    "throw",
                    field("excNo", $._expression),
                ),
                seq(
                    "(",
                    field("condition", $._expression),
                    ",",
                    field("excNo", $._expression),
                    ")",
                ),
            ),
        ),

    catch_clause: $ =>
        seq(
            optional(
                seq(
                    "(",
                    field("catch_var1", $.identifier),
                    optional(seq(",", field("catch_var2", $.identifier))),
                    ")",
                ),
            ),
            field("catch_body", $.block_statement),
        ),

    try_catch_statement: $ =>
        seq("try", field("try_body", $.block_statement), "catch", field("catch", $.catch_clause)),

    empty_statement: $ => ";",

    expression_statement: $ => $._expression,

    // ----------------------------------------------------------
    // expressions

    _expression: $ =>
        choice(
            $.var_declaration_lhs,
            $.assignment,
            $.set_assignment,
            $.ternary_operator,
            $.binary_operator,
            $.unary_operator,
            $.lazy_expression,
            $.cast_as_operator,
            $.is_type_operator,
            $.not_null_operator,
            $.dot_access,
            $.function_call,
            $.generic_instantiation,
            $.parenthesized_expression,
            $.match_expression,
            $.object_literal,
            $.tensor_expression,
            $.typed_tuple,
            $.lambda_expression,
            $.number_literal,
            $.string_literal,
            $.boolean_literal,
            $.null_literal,
            $.underscore,
            $.identifier,
        ),

    assignment: $ =>
        prec.right(10, seq(field("left", $._expression), "=", field("right", $._expression))),

    set_assignment: $ =>
        prec.right(
            10,
            seq(
                field("left", $._expression),
                field(
                    "operator_name",
                    choice("+=", "-=", "*=", "/=", "%=", "<<=", ">>=", "&=", "|=", "^="),
                ),
                field("right", $._expression),
            ),
        ),

    ternary_operator: $ =>
        prec.right(
            10,
            seq(
                field("condition", $._expression),
                "?",
                field("consequence", $._expression),
                ":",
                field("alternative", $._expression),
            ),
        ),

    _brackets_lt_gt: _ => choice("<", ">"), // extracted specially to resolve conflicts between `<` and `f<int>`
    _comparison_lt_gt: $ =>
        prec.left(15, seq($._expression, field("operator_name", $._brackets_lt_gt), $._expression)),

    binary_operator: $ =>
        choice(
            prec.right(10, seq($._expression, field("operator_name", "??"), $._expression)),
            prec.left(
                13,
                seq($._expression, field("operator_name", choice("&&", "||")), $._expression),
            ),
            prec.left(
                14,
                seq($._expression, field("operator_name", choice("&", "|", "^")), $._expression),
            ),
            prec.left(
                15,
                seq(
                    $._expression,
                    field("operator_name", choice("==", "!=", "<=", ">=", "<=>")),
                    $._expression,
                ),
            ),
            $._comparison_lt_gt,
            prec.left(
                17,
                seq(
                    $._expression,
                    field("operator_name", choice("<<", ">>", "~>>", "^>>")),
                    $._expression,
                ),
            ),
            prec.left(
                20,
                seq($._expression, field("operator_name", choice("-", "+")), $._expression),
            ),
            prec.left(
                30,
                seq(
                    $._expression,
                    field("operator_name", choice("*", "/", "%", "~/", "^/")),
                    $._expression,
                ),
            ),
        ),

    unary_operator: $ =>
        choice(
            prec.left(
                75,
                seq(
                    field("operator_name", choice("!", "~", "-", "+")),
                    field("argument", $._expression),
                ),
            ),
        ),

    lazy_expression: $ => choice(prec.right(5, seq("lazy", field("argument", $._expression)))),

    cast_as_operator: $ =>
        prec(40, seq(field("expr", $._expression), "as", field("casted_to", $._type_hint))),
    is_type_operator: $ =>
        prec(
            40,
            seq(
                field("expr", $._expression),
                field("operator", choice("is", "!is")),
                field("rhs_type", $._type_hint),
            ),
        ),

    dot_access: $ =>
        prec(
            80,
            seq(
                field("obj", $._expression),
                ".",
                field("field", choice($.identifier, $.numeric_index)), // for method call, dot_access is wrapped into function_call, "field" actually means method name
            ),
        ),

    not_null_operator: $ => prec(90, seq(field("inner", $._expression), "!")),

    function_call: $ =>
        prec.left(
            90,
            seq(
                field("callee", $._expression), // callee can be generic_instantiation or dot_access
                field("arguments", $.argument_list),
            ),
        ),
    argument_list: $ => seq("(", commaSep($.call_argument), optional(","), ")"),
    call_argument: $ => seq(optional("mutate"), field("expr", $._expression)),

    generic_instantiation: $ =>
        prec(
            10,
            seq(field("expr", $._expression), field("instantiationTs", $.instantiationT_list)),
        ),
    instantiationT_list: $ =>
        prec.dynamic(
            1,
            seq(
                // prec.dynamic is important
                "<",
                field("types", commaSep1($._type_hint)),
                optional(","),
                ">",
            ),
        ),

    match_statement: $ => prec(99, $.match_expression),
    match_expression: $ =>
        prec.right(
            seq(
                "match",
                "(",
                field("expr", $._expression),
                ")",
                optional(field("body", $.match_body)),
            ),
        ),
    match_body: $ => seq("{", optional(repeat1($.match_arm)), "}"),
    match_arm: $ =>
        seq(
            choice(
                field("pattern_type", $._type_hint),
                field("pattern_expr", $._expression),
                field("pattern_else", "else"),
            ),
            "=>",
            field(
                "body",
                choice(
                    field("block", $.block_statement),
                    field("return", $.return_statement),
                    field("throw", $.throw_statement),
                    field("expr", $._expression),
                ),
            ),
            optional(","), // todo now `match (...) { 1 => 1 2 => 2 }` is ok, but actually comma is required
        ),

    object_literal: $ =>
        prec.dynamic(
            99,
            seq(optional(field("type", $._type_hint)), field("arguments", $.object_literal_body)),
        ),
    object_literal_body: $ => seq("{", commaSep($.instance_argument), optional(","), "}"),
    instance_argument: $ =>
        seq(
            field("name", $.identifier),
            optional(seq(":", optional(field("value", $._expression)))),
        ),

    parenthesized_expression: $ => seq("(", field("inner", $._expression), optional(","), ")"),
    tensor_expression: $ =>
        choice(seq("(", ")"), seq("(", commaSep2($._expression), optional(","), ")")),
    typed_tuple: $ =>
        seq(
            optional(field("type", $._type_hint)),
            "[",
            field("elements", commaSep($._expression)),
            optional(","),
            "]",
        ),

    lambda_expression: $ =>
        prec.right(
            seq(
                "fun",
                field("parameters", $.parameter_list),
                optional(seq(":", field("return_type", optional($._type_hint)))),
                optional(field("body", $._function_body)),
            ),
        ),

    // ----------------------------------------------------------
    // type system

    _type_hint: $ =>
        prec.dynamic(
            100,
            choice(
                alias($.identifier, $.type_identifier),
                $.type_instantiatedTs,
                $.tensor_type,
                $.tuple_type,
                $.parenthesized_type,
                $.fun_callable_type,
                $.nullable_type,
                $.union_type,
                $.null_literal,
            ),
        ),

    type_instantiatedTs: $ =>
        prec.dynamic(
            104,
            seq(
                field("name", alias($.identifier, $.type_identifier)),
                field("arguments", $.instantiationT_list),
            ),
        ),

    tensor_type: $ =>
        prec.dynamic(
            103,
            choice(seq("(", ")"), seq("(", commaSep2($._type_hint), optional(","), ")")),
        ),
    tuple_type: $ => prec(103, seq("[", commaSep1($._type_hint), optional(","), "]")),
    parenthesized_type: $ => prec(103, seq("(", field("inner", $._type_hint), ")")),

    fun_callable_type: $ =>
        prec.right(
            101,
            seq(field("param_types", $._type_hint), "->", field("return_type", $._type_hint)),
        ),
    nullable_type: $ => prec.right(110, seq(field("inner", $._type_hint), "?")),
    union_type: $ =>
        prec.right(
            102,
            seq(
                optional("|"),
                field("lhs", $._type_hint),
                "|",
                field("rhs", $._type_hint),
                optional("|"),
            ),
        ),

    // ----------------------------------------------------------
    // common constructions

    number_literal: $ =>
        token(choice(seq("0x", /[0-9a-fA-F_]+/), seq("0b", /[01_]+/), /[0-9][0-9_]*/)),
    string_literal: $ =>
        token(
            choice(
                seq('"""', repeat(choice(/[^"]/, /"[^"]/, /""[^"]/)), '"""'),
                seq('"', repeat(choice(/[^"\\\n]/, /\\./)), '"'),
            ),
        ),
    boolean_literal: $ => choice("true", "false"),
    null_literal: $ => "null",
    underscore: $ => "_",
    identifier: $ => /`[^`]+`|[a-zA-Z$_][a-zA-Z0-9$_]*/,
    numeric_index: $ => /[0-9]+/,

    // http://stackoverflow.com/questions/13014947/regex-to-match-a-c-style-multiline-comment/36328890#36328890
    comment: $ => token(choice(seq("//", /[^\r\n]*/), seq("/*", /[^*]*\*+([^/*][^*]*\*+)*/, "/"))),
}

module.exports = grammar({
    name: "tolk",

    conflicts: $ => [
        [$.instantiationT_list, $._brackets_lt_gt],
        [$._comparison_lt_gt, $.binary_operator, $.generic_instantiation],
        [$._expression, $.instance_argument],
        [$._expression, $._type_hint],
        [$.type_parameter, $._type_hint],
        [$.type_instantiatedTs, $._type_hint],
        [$._expression, $.type_instantiatedTs],
        [$.tensor_type, $.tensor_expression],
        [$.union_type],
        [$.block_statement, $.object_literal_body],
    ],

    extras: $ => [/\s/, $.comment],

    word: $ => $.identifier,

    rules: TOLK_GRAMMAR,
})
