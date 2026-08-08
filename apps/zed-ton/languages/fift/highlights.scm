; Editor highlighting for Fift.

(comment) @comment
(string) @string

[
  (number)
  (hex_literal)
  (stack_ref)
  (stack_index)
] @number

(slice_literal) @string.special

[
  "include"
  "PROGRAM{"
  "END>c"
  "DECLPROC"
  "DECLMETHOD"
  "DECLGLOBVAR"
  "PROC:<{"
  "PROCINLINE:<{"
  "PROCREF:<{"
  "METHOD:<{"
  "IF:<{"
  "ELSE<{"
  "IFJMP:<{"
  "WHILE:<{"
  "}>DO<{"
  "REPEAT:<{"
  "UNTIL:<{"
  "CALLDICT"
  "INLINECALLDICT"
] @keyword

(proc_declaration
  name: (identifier) @function)

(proc_definition
  name: (identifier) @function)

(proc_inline_definition
  name: (identifier) @function)

(proc_ref_definition
  name: (identifier) @function)

(method_declaration
  name: (identifier) @function.method)

(method_definition
  name: (identifier) @function.method)

(global_var
  name: (identifier) @variable)

(instruction
  (identifier) @function)

(negative_identifier
  (identifier) @function)

"-" @operator
