; An export taken from another module's file: surface claimed away from its definition.
(export_statement
  (export_clause)
  source: (string (string_fragment) @from))

; A star re-export forwards every name unseen: a blanket, even on a surface.
(export_statement
  "*"
  source: (string (string_fragment) @star))
(export_statement
  (namespace_export)
  source: (string (string_fragment) @star))
