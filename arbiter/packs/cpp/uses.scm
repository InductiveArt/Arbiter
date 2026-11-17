; A call is a use only of what the file includes; the quoted form names project
; headers. A named type is a use wherever it appears.
(call_expression function: (identifier) @use)
(call_expression function: (qualified_identifier name: (identifier) @use))
(call_expression function: (field_expression field: (field_identifier) @use))
((type_identifier) @use)
(preproc_include path: (string_literal) @include)
