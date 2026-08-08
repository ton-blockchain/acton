; Editor text objects for TASM.

(code) @function.around
(code
  "{"
  (_)* @function.inside
  "}")
((comment)+) @comment.around
