; A named import, with the module path it names.
(import_statement
  (import_clause (named_imports (import_specifier name: (identifier) @name)))
  source: (string (string_fragment) @source))

; A name taken under another: readable only by whoever knows the alias.
((import_specifier alias: (identifier) @alias) @aliased)
