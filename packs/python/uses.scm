; A name taken from another module, with the module it is taken from.
(import_from_statement
  module_name: (dotted_name) @module
  name: (dotted_name) @name)

; An aliased name still reaches its module; the alias is judged apart.
(import_from_statement
  module_name: (dotted_name) @module
  name: (aliased_import name: (dotted_name) @name))

; A module imported whole, and the dotted chains that may reach into it.
(import_statement name: (dotted_name) @plain)
(import_statement name: (aliased_import name: (dotted_name) @plain))
((attribute) @chain)

; A name taken under another: readable only by whoever knows the alias.
((aliased_import) @alias)
