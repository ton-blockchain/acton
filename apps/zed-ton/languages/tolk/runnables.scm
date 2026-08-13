; Editor runnable entry points for Acton scripts and tests.

(function_declaration
  name: (identifier) @run
  (#eq? @run "main")
  (#set! tag acton-script)) @_

(get_method_declaration
  name: (identifier) @run @acton_test_name
  (#match? @acton_test_name "^`test(?: |`)")
  (#set! tag acton-test)) @_
