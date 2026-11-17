; A module's own top-level names: what the file offers to an importer.
(module (function_definition name: (identifier) @def))
(module (class_definition name: (identifier) @def))
(module (decorated_definition definition: (function_definition name: (identifier) @def)))
(module (decorated_definition definition: (class_definition name: (identifier) @def)))
(module (expression_statement (assignment left: (identifier) @def)))
