; What a header offers: functions at the top level or inside a namespace, types
; with their bodies, methods, enums and aliases. A forward declaration offers
; nothing, so a type counts only with its body.
(translation_unit (declaration declarator: (function_declarator declarator: (identifier) @def)))
(namespace_definition body: (declaration_list (declaration declarator: (function_declarator declarator: (identifier) @def))))
(translation_unit (function_definition declarator: (function_declarator declarator: (identifier) @def)))
(namespace_definition body: (declaration_list (function_definition declarator: (function_declarator declarator: (identifier) @def))))
(class_specifier name: (type_identifier) @def body: (_))
(struct_specifier name: (type_identifier) @def body: (_))
(enum_specifier name: (type_identifier) @def body: (_))
(alias_declaration name: (type_identifier) @def)
(type_definition declarator: (type_identifier) @def)
(field_declaration declarator: (function_declarator declarator: (field_identifier) @def))
(function_definition declarator: (function_declarator declarator: (field_identifier) @def))
