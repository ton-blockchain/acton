; Editor text objects for Tolk.

[
  (function_declaration)
  (method_declaration)
  (get_method_declaration)
] @function.around

(function_declaration
  body: (block_statement
    "{"
    (_)* @function.inside
    "}"))

(method_declaration
  body: (block_statement
    "{"
    (_)* @function.inside
    "}"))

(get_method_declaration
  body: (block_statement
    "{"
    (_)* @function.inside
    "}"))

[
  (contract_declaration)
  (struct_declaration)
  (enum_declaration)
  (type_alias_declaration)
] @class.around

(contract_declaration
  body: (contract_body
    "{"
    (_)* @class.inside
    "}"))

(struct_declaration
  body: (struct_body
    "{"
    (_)* @class.inside
    "}"))

(enum_declaration
  body: (enum_body
    "{"
    (_)* @class.inside
    "}"))

((comment)+) @comment.around
