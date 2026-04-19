"do" @keyword
"if" @keyword
"as" @keyword
"is" @keyword
"!is" @keyword
"fun" @keyword
"asm" @keyword
"get" @keyword
"try" @keyword
"var" @keyword
"val" @keyword
"lazy" @keyword
"else" @keyword
"type" @keyword
"enum" @keyword
"struct" @keyword
"contract" @keyword
"readonly" @keyword
"private" @keyword
"true" @keyword
"tolk" @keyword
"const" @keyword
"false" @keyword
"throw" @keyword
"redef" @keyword
"while" @keyword
"catch" @keyword
"match" @keyword
"return" @keyword
"assert" @keyword
"import" @keyword
"global" @keyword
"repeat" @keyword
"mutate" @keyword
(null_literal) @keyword
(builtin_specifier) @keyword

"=" @operator
"+=" @operator
"-=" @operator
"*=" @operator
"/=" @operator
"%=" @operator
"<<=" @operator
">>=" @operator
"&=" @operator
"|=" @operator
"^=" @operator

"==" @operator
"<" @operator
">" @operator
"<=" @operator
">=" @operator
"!=" @operator
"<=>" @operator
"<<" @operator
">>" @operator
"~>>" @operator
"^>>" @operator
"-" @operator
"+" @operator
"|" @operator
"^" @operator
"*" @operator
"/" @operator
"%" @operator
"~/" @operator
"^/" @operator
"&" @operator
"~" @operator
"." @operator
"!" @operator
"&&" @operator
"||" @operator
"??" @operator

"->" @operator


(string_literal) @string
(number_literal) @number
(boolean_literal) @number

(annotation) @attribute

(function_declaration
  name: (identifier) @function)
(method_declaration
  name: (identifier) @function)
(get_method_declaration
  name: (identifier) @function)
(function_call
  callee: (identifier) @function)
(function_call
  callee: (dot_access (identifier) (identifier) @function))
(dot_access
  field: (identifier) @variable)

(type_identifier) @type

(identifier) @variable

(comment) @comment
