; Editor highlighting for TASM.

(comment) @comment
(string_literal) @string

[
  (integer_literal)
  (stack_element)
  (control_register)
] @number

[
  (hex_literal)
  (bin_literal)
  (boc_literal)
] @string.special

[
  (kw_ref)
  (kw_embed)
  (kw_exotic)
  (kw_library)
] @keyword

(instruction
  name: (identifier) @function)

"=>" @operator

[
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket
