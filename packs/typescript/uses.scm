; A named import, with the module path it names.
(import_statement
  (import_clause (named_imports (import_specifier name: (identifier) @name)))
  source: (string (string_fragment) @source))

; A name taken under another: readable only by whoever knows the alias.
((import_specifier alias: (identifier) @alias) @aliased)

; A dynamic import of a literal module: which names it takes resolves only at
; run time, so every name the module offers counts as used.
(call_expression
  function: (import)
  arguments: (arguments (string (string_fragment) @dynamic)))

; A dynamic import of a computed value, which no reader or judge can resolve.
(call_expression
  function: (import)
  arguments: (arguments [(identifier) (member_expression) (template_string) (binary_expression) (call_expression) (subscript_expression)] @computed))
