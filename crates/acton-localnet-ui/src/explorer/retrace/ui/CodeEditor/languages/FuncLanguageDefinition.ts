import {languages} from "monaco-editor"

export const funcLanguageDefinition: languages.IMonarchLanguage = {
  defaultToken: "invalid",

  keywords: [
    "forall",
    "return",
    "if",
    "ifnot",
    "else",
    "elseif",
    "elseifnot",
    "repeat",
    "do",
    "until",
    "while",
    "global",
    "const",
    "asm",
    "impure",
    "inline",
    "inline_ref",
    "method_id",
    "type",
    "null",
    "throw",
    "throw_if",
    "throw_unless",
    "true",
    "false",
    "empty_tuple",
  ],

  typeKeywords: ["int", "cell", "slice", "builder", "cont", "tuple", "var"],

  directives: [
    "#include",
    "#pragma",
    "version",
    "not-version",
    "allow-post-modification",
    "compute-asm-ltr",
  ],

  operators: [
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
    "==",
    "!=",
    "<",
    ">",
    "<=",
    ">=",
    "<=>",
    "<<",
    ">>",
    "~>>",
    "^>>",
    "+",
    "-",
    "|",
    "^",
    "*",
    "/",
    "%",
    "~/",
    "^/",
    "~%",
    "^%",
    "/%",
    "&",
    "~",
    ".",
    "?",
    ":",
    "->",
    "=>",
  ],

  symbols: /[=><!~?:&|+\-*/^%]+/,

  tokenizer: {
    root: [
      // Comments
      [/;;.*$/, "comment"],
      [/\{-/, {token: "comment", next: "@comment"}],

      // ASM blocks with triple quotes
      [/"""/, {token: "string.asm", next: "@asmblock"}],

      // Directives
      [/(#include|#pragma)\b/, "keyword.directive"],
      [/\b(version|not-version|allow-post-modification|compute-asm-ltr)\b/, "keyword.directive"],

      // Version identifiers in pragmas
      [/(>=|<=|=|>|<|\^)?([0-9]+)(.[0-9]+)?(.[0-9]+)?/, "number.version"],

      // Method identifiers with % prefix
      [/%[a-zA-Z_][a-zA-Z0-9_]*/, "identifier.function"],

      // Special function identifiers with $ prefix and apostrophes
      [/\$[a-zA-Z_][a-zA-Z0-9_$']*/, "identifier.special"],

      // Identifiers in backticks
      [/`[^`]+`/, "identifier.backtick"],

      // Constants in UPPER_CASE
      [/\b[A-Z][A-Z0-9_]*\b/, "identifier.constant"],

      // Function calls with parentheses
      [/[a-zA-Z_][a-zA-Z0-9_'?]*(?=\()/, "identifier.function"],

      // Tilde operators (function calls)
      [/~[a-zA-Z_][a-zA-Z0-9_?]*/, "identifier.function"],
      // Method names (with dot prefix)
      [/\.[a-zA-Z_][a-zA-Z0-9_?]*/, "identifier.function"],

      // Regular identifiers and keywords
      [
        /[a-zA-Z_][a-zA-Z0-9_']*/,
        {
          cases: {
            "@keywords": "keyword",
            "@typeKeywords": "type",
            "@default": "identifier",
          },
        },
      ],

      // Numbers
      [/-?0x[0-9a-fA-F]+/, "number.hex"],
      [/-?\d+/, "number"],

      // Strings
      [/"[^"]*"[Hhcu]/, "string.number"],
      [/"[^"]*"[sa]/, "string.slice"],
      [/"[^"]*"/, "string"],

      // Operators including tilde
      [/~/, "operator"],
      [
        /@symbols/,
        {
          cases: {
            "@operators": "operator",
            "@default": "",
          },
        },
      ],

      // Delimiters
      [/[{}]/, "@brackets"],
      [/[[\]]/, "delimiter.square"],
      [/[()]/, "delimiter.parenthesis"],
      [/;/, "delimiter"],
      [/,/, "delimiter"],

      // Special symbols
      [/_/, "keyword.underscore"],

      // Whitespace
      [/\s+/, "white"],
    ],

    asmblock: [
      [/"""/, {token: "string.asm", next: "@pop"}],
      [/[^"]+/, "string.asm"],
      [/"/, "string.asm"],
    ],

    comment: [
      [/[^-{]+/, "comment"],
      [/\{-/, {token: "comment", next: "@push"}],
      [/-\}/, {token: "comment", next: "@pop"}],
      [/[-{]/, "comment"],
    ],
  },
}
