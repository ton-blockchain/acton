; Zed-specific highlighting for Tolk.
; Keep this query in sync with the bundled `grammars/tolk.wasm`.

[
  "tolk"
  "import"
  "global"
  "const"
  "fun"
  "get"
  "asm"
  "return"
  "if"
  "else"
  "while"
  "do"
  "repeat"
  "try"
  "catch"
  "throw"
  "assert"
  "match"
  "var"
  "val"
  "mutate"
  "redef"
  "lazy"
  "struct"
  "enum"
  "type"
  "private"
  "readonly"
] @keyword

[
  "true"
  "false"
] @boolean

(null_literal) @constant.builtin
(builtin_specifier) @keyword

[
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "<<="
  ">>="
  "&="
  "|="
  "^="
  "=="
  "<"
  ">"
  "<="
  ">="
  "!="
  "<=>"
  "<<"
  ">>"
  "~>>"
  "^>>"
  "-"
  "+"
  "|"
  "^"
  "*"
  "/"
  "%"
  "~/"
  "^/"
  "&"
  "~"
  "."
  "!"
  "&&"
  "||"
  "is"
  "!is"
] @operator

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  ";"
  ":"
  "->"
] @punctuation.delimiter

(string_literal) @string
(number_literal) @number
(comment) @comment

;(identifier) @variable

(annotation
  name: (identifier) @attribute)

(contract_declaration
  name: (identifier) @type)
(struct_declaration
  name: (identifier) @type)
(enum_declaration
  name: (identifier) @type)
(type_alias_declaration
  name: (identifier) @type)
(type_identifier) @type

(enum_member_declaration
  name: (identifier) @constant)

(function_declaration
  name: (identifier) @function)
(method_declaration
  name: (identifier) @function.method)
(get_method_declaration
  name: (identifier) @function.method)

(parameter_declaration
  name: (identifier) @variable.parameter)

(instance_argument
  name: (identifier) @property)

(struct_field_declaration
  name: (identifier) @property)

(contract_field
  name: (identifier) @property)

(dot_access
  field: (identifier) @property)

(function_call
  callee: (identifier) @function)
(function_call
  callee: (dot_access
    field: (identifier) @function.method))
(function_call
  callee: (generic_instantiation
    expr: (identifier) @function))
(function_call
  callee: (generic_instantiation
    expr: (dot_access
      field: (identifier) @function.method)))

((identifier) @constant
  (#match? @constant "^[A-Z][A-Z0-9_]*$"))
