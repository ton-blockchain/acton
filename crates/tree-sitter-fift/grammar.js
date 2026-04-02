/**
 * @file Fift grammar for tree-sitter
 * @author TON Core
 * @author TON Studio
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
    name: "fift",

    extras: $ => [/\s/, $.comment],

    rules: {
        source_file: $ => seq(optional($.include_directive), $.program),

        include_directive: () => seq('"', /[^"]+/, '"', "include"),

        comment: _ => token(seq("//", /[^\n]*/)),

        program: $ => seq("PROGRAM{", repeat($.declaration), repeat($.definition), "END>c"),

        declaration: $ => choice($.proc_declaration, $.method_declaration, $.global_var),

        proc_declaration: $ => seq("DECLPROC", field("name", $.identifier)),

        method_declaration: $ =>
            seq(field("id", $.number), "DECLMETHOD", field("name", $.identifier)),

        global_var: $ => seq("DECLGLOBVAR", field("name", $.identifier)),

        definition: $ =>
            choice(
                $.proc_definition,
                $.proc_inline_definition,
                $.proc_ref_definition,
                $.method_definition,
            ),

        proc_definition: $ =>
            seq(field("name", $.identifier), "PROC:<{", repeat($.instruction), "}>"),

        proc_inline_definition: $ =>
            seq(field("name", $.identifier), "PROCINLINE:<{", repeat($.instruction), "}>"),

        proc_ref_definition: $ =>
            seq(field("name", $.identifier), "PROCREF:<{", repeat($.instruction), "}>"),

        method_definition: $ =>
            seq(field("name", $.identifier), "METHOD:<{", repeat($.instruction), "}>"),

        instruction: $ =>
            choice(
                $.identifier,
                $.negative_identifier,
                $.number,
                $.string,
                $.if_statement,
                $.ifjmp_statement,
                $.while_statement,
                $.repeat_statement,
                $.until_statement,
                $.proc_call,
                $.slice_literal,
                $.hex_literal,
                $.stack_ref,
                $.stack_op,
                $.instruction_block,
            ),

        if_statement: $ =>
            seq(
                "IF:<{",
                repeat($.instruction),
                "}>",
                optional(seq("ELSE<{", repeat($.instruction), "}>")),
            ),

        ifjmp_statement: $ => seq("IFJMP:<{", repeat($.instruction), "}>"),

        while_statement: $ =>
            seq("WHILE:<{", repeat($.instruction), "}>DO<{", repeat($.instruction), "}>"),

        repeat_statement: $ => seq("REPEAT:<{", repeat($.instruction), "}>"),

        until_statement: $ => seq("UNTIL:<{", repeat($.instruction), "}>"),

        proc_call: $ => seq($.identifier, choice("CALLDICT", "INLINECALLDICT")),

        instruction_block: $ => seq("<{", repeat($.instruction), "}>"),

        slice_literal: () =>
            choice(
                seq("b{", /[01]+/, "}"),
                seq("x{", /[0-9a-fA-F_]+/, "}"),
                seq("B{", /[0-9a-fA-F_]+/, "}"),
            ),

        hex_literal: () => /0[xX][0-9a-fA-F]+/,

        identifier: () => /[~a-zA-Z$_%?][a-zA-Z0-9$_?~.()]*/,

        negative_identifier: $ => seq("-", $.identifier),

        number: () => /-?\d+/,

        stack_ref: () => seq("s", "(", /-?\d+/, ")"),

        stack_op: $ => seq($.stack_index, $.stack_index, optional($.stack_ref), $.identifier),

        stack_index: () => seq("s", /[0-9]+/),

        string: () => seq('"', /[^"]*/, '"'),
    },
})
