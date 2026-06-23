import {languages} from "monaco-editor"

export const tolkLanguageDefinition: languages.IMonarchLanguage = {
  defaultToken: "invalid",

  keywords: [
    "tolk",
    "import",
    "global",
    "const",
    "type",
    "struct",
    "fun",
    "get",
    "var",
    "val",
    "return",
    "if",
    "else",
    "while",
    "repeat",
    "do",
    "break",
    "continue",
    "throw",
    "assert",
    "try",
    "catch",
    "match",
    "as",
    "is",
    "lazy",
    "mutate",
    "redef",
    "builtin",
    "asm",
    "true",
    "false",
    "null",
    "self",
  ],

  typeKeywords: [
    "int",
    "bool",
    "cell",
    "slice",
    "builder",
    "continuation",
    "tuple",
    "address",
    "never",
    "coins",
    "map",
    "void",
  ],

  operators: [
    "=",
    "+=",
    "-=",
    "*=",
    "/=",
    "%=",
    "<<=",
    ">>=",
    "&=",
    "|=",
    "^=",
    "==",
    "!=",
    "<=",
    ">=",
    "<=>",
    "&&",
    "||",
    "&",
    "|",
    "^",
    "<<",
    ">>",
    "~>>",
    "^>>",
    "-",
    "+",
    "*",
    "/",
    "%",
    "~/",
    "^/",
    "!",
    "~",
    "->",
    "?",
    ":",
    ".",
    "<",
    ">",
  ],

  symbols: /[=><!~?:&|+\-*/^%()[\]{}.,;]+/,

  tokenizer: {
    root: [
      // Comments
      [/\/\/.*$/, "comment"],
      [/\/\*/, {token: "comment", next: "@comment"}],

      // Triple-quoted strings (multiline)
      [/"""/, {token: "string", next: "@stringTriple"}],

      // Regular string literals
      [/"(?:[^"\\\n]|\\.)*"/, "string"],

      // uint32 or int123
      [/u?int\d{1,3}/, "type"],
      // varuint32 or varint16
      [/varu?int\d{1,2}/, "type"],
      // bits256 or bytes32
      [/((bits)|(bytes))\d{1,3}/, "type"],

      // Annotations
      [/@[a-zA-Z_][a-zA-Z0-9_]*/, "annotation"],

      // Numbers
      [/0x[0-9a-fA-F]+/, "number.hex"],
      [/0b[01]+/, "number.binary"],
      [/\d+/, "number"],

      // Version numbers in tolk directives
      [/(\d+)(\.\d+)?(\.\d+)?/, "number.version"],

      // Identifiers in backticks
      [/`[^`]+`/, "identifier.backtick"],

      // Function calls foo() or foo<...
      [/[a-zA-Z_][a-zA-Z0-9_$]*(?=[(<])/, "identifier.function"],

      // Member access: dot as its own token, then color the name
      [/\.(?=[A-Za-z_][A-Za-z0-9_]*)/, {token: "delimiter.dot", next: "@member"}],

      // Type identifiers (capitalized)
      [/[A-Z][a-zA-Z0-9_]*/, "type.identifier"],

      // Regular identifiers and keywords
      [
        /[a-zA-Z$_][a-zA-Z0-9$_]*/,
        {
          cases: {
            "@keywords": "keyword",
            "@typeKeywords": "type",
            "@default": "identifier",
          },
        },
      ],

      // Operators
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

      // Underscore
      [/_/, "keyword.underscore"],

      // Whitespace
      [/\s+/, "white"],
    ],

    // Multiline triple-quoted strings:
    // - closes strictly on """
    // - supports backslash escapes like \" or \n inside
    // - allows single or double " characters inside without closing
    stringTriple: [
      [/"""/, {token: "string", next: "@pop"}],
      [/\\./, "string.escape"],
      [/[^\\"]+/, "string"],
      [/"/, "string"],
    ],

    comment: [
      [/[^/*]+/, "comment"],
      [/\/\*/, {token: "comment", next: "@push"}],
      [/\*\//, {token: "comment", next: "@pop"}],
      [/[/*]/, "comment"],
    ],

    member: [
      // method call: .foo() or .foo<...
      [/[A-Za-z_][A-Za-z0-9_]*(?=[(<])/, {token: "identifier.function", next: "@pop"}],
      // field access: .foo
      [/[A-Za-z_][A-Za-z0-9_]*/, {token: "identifier.field", next: "@pop"}],
    ],
  },
}
