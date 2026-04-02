/**
 * @file TASM grammar for tree-sitter
 * @author TON Core
 * @license MIT
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

const TASM_GRAMMAR = {
  source_file: $ => optional($.instructions),

  // ----------------------------------------------------------
  // top-level structures

  instructions: $ => repeat1(choice($.instruction, $.explicit_ref, $.embed_slice, $.exotic)),

  instruction: $ => seq(field("name", $.identifier), repeat(field("args", $.argument))),

  argument: $ =>
    choice(
      $.integer_literal,
      $.data_literal,
      $.code,
      $.dictionary,
      $.stack_element,
      $.control_register,
    ),

  stack_element: _ => token(seq("s", optional("-"), /[0-9](?:_?[0-9])*/)),
  control_register: _ => token(seq("c", /[0-9](?:_?[0-9])*/)),

  code: $ => seq("{", optional(field("instructions", $.instructions)), "}"),

  dictionary: $ => seq("[", repeat(field("entries", $.dictionary_entry)), "]"),
  dictionary_entry: $ => seq(field("id", $.integer_literal), "=>", field("code", $.code)),

  explicit_ref: $ => seq($.kw_ref, field("code", $.code)),
  embed_slice: $ => seq($.kw_embed, field("data", $.data_literal)),
  exotic: $ => seq($.kw_exotic, field("lib", choice($.exotic_library, $.default_exotic))),
  exotic_library: $ => seq($.kw_library, field("data", $.data_literal)),
  default_exotic: $ => field("data", $.data_literal),

  // ----------------------------------------------------------
  // literals and atoms

  identifier: _ => token(prec(-1, /[a-zA-Z_][a-zA-Z0-9_]*/)),

  data_literal: $ => choice($.hex_literal, $.bin_literal, $.boc_literal, $.string_literal),
  hex_literal: _ => token(seq("x{", /[0-9a-fA-F]*/, optional("_"), "}")),
  bin_literal: _ => token(seq("b{", /[01]*/, "}")),
  boc_literal: _ => token(seq("boc{", /[0-9a-fA-F]*/, "}")),
  string_literal: _ => token(seq('"', /[^"\\]*/, '"')),

  integer_literal: _ =>
    token(
      seq(
        optional("-"),
        choice(
          /0[xX][0-9a-fA-F](?:_?[0-9a-fA-F])*/,
          /0[bB][01](?:_?[01])*/,
          /0[oO][0-7](?:_?[0-7])*/,
          /[0-9](?:_?[0-9])*/,
        ),
      ),
    ),

  kw_ref: _ => token(prec(1, "ref")),
  kw_embed: _ => token(prec(1, "embed")),
  kw_exotic: _ => token(prec(1, "exotic")),
  kw_library: _ => token(prec(1, "library")),

  comment: _ => token(seq("//", /[^\r\n]*/)),
}

module.exports = grammar({
  name: "tasm",

  extras: $ => [/\s/, $.comment],

  word: $ => $.identifier,

  rules: TASM_GRAMMAR,
})
