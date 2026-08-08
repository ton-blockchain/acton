; Editor outline for top-level Tolk declarations.

(contract_declaration
  "contract" @context
  name: (identifier) @name) @item

(struct_declaration
  "struct" @context
  name: (identifier) @name) @item

(enum_declaration
  "enum" @context
  name: (identifier) @name) @item

(type_alias_declaration
  "type" @context
  name: (identifier) @name) @item

(global_var_declaration
  "global" @context
  name: (identifier) @name) @item

(constant_declaration
  "const" @context
  name: (identifier) @name) @item

(function_declaration
  "fun" @context
  name: (identifier) @name) @item

(method_declaration
  "fun" @context
  receiver: (method_receiver) @context
  name: (identifier) @name) @item

(get_method_declaration
  "get" @context
  "fun"? @context
  name: (identifier) @name) @item

(enum_member_declaration
  name: (identifier) @name) @item

(struct_field_declaration
  name: (identifier) @name) @item
