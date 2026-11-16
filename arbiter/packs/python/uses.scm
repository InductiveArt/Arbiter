; A name taken from another module, with the module it is taken from.
(import_from_statement
  module_name: (dotted_name) @module
  name: (dotted_name) @name)

; A star import: every name its module offers, by whichever the file uses.
(import_from_statement
  module_name: (dotted_name) @star
  (wildcard_import))

; An aliased name still reaches its module; the alias is read apart.
(import_from_statement
  module_name: (dotted_name) @module
  name: (aliased_import name: (dotted_name) @name))

; A module imported whole, and the dotted chains that may reach into it.
(import_statement name: (dotted_name) @plain)
(import_statement name: (aliased_import name: (dotted_name) @plain))
((attribute) @chain)

; A name taken under another: readable only by whoever knows the alias.
((aliased_import) @alias)

; A run-time import: a literal module reaches like an import written out,
; a computed one is named, since no reader can resolve it.
(call
  function: [(identifier) (attribute)] @loader
  arguments: (argument_list . (string) @dynamic))
(call
  function: [(identifier) (attribute)] @loader
  arguments: (argument_list . [(identifier) (attribute) (binary_operator) (call) (subscript) (conditional_expression)] @computed))
