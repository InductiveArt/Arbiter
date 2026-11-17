; A name reached through a path, with the path leading to it: a use declaration,
; a scoped call or value, or a scoped type.
(use_declaration argument: (scoped_identifier path: (_) @declared_module name: (identifier) @name))
(use_declaration argument: (scoped_use_list path: (_) @declared_module list: (use_list (identifier) @name)))
(scoped_identifier path: (_) @module name: (identifier) @name)
(scoped_type_identifier path: (_) @module name: (type_identifier) @name)

; A name taken under another: readable only by whoever knows the alias.
((use_as_clause) @alias)
