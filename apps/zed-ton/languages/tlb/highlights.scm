; Editor highlighting for TL-B.

(comment) @comment

[
  (number)
  (binary_number)
  (hex)
] @number

(builtin_field) @type.builtin
(type_identifier) @type

(constructor_
  name: (identifier) @function)

(field_builtin
  name: (identifier) @variable.parameter)

(field_named
  name: (identifier) @property)

(field_named_anon_ref
  (identifier) @property)

[
  "!"
  "$"
  "#"
  "~"
  "<="
  ">="
  "!="
  "="
  "<"
  ">"
  "+"
  "*"
  "?"
  "."
  "^"
  "##"
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
  ":"
  ";"
] @punctuation.delimiter
