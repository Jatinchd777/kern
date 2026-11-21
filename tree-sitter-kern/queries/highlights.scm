"fn" @keyword.function
"let" @keyword
"mut" @keyword
"if" @keyword.conditional
"else" @keyword.conditional
"while" @keyword.repeat
"loop" @keyword.repeat
"for" @keyword.repeat
"break" @keyword
"continue" @keyword
"return" @keyword.return
"struct" @keyword
"enum" @keyword
"match" @keyword.conditional
"use" @keyword.import
"print" @function.builtin
"println" @function.builtin
"cin" @constant.builtin

(primitive_type) @type.builtin
(type_identifier) @type
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function.call)
(parameter name: (identifier) @variable.parameter)
(let_statement name: (identifier) @variable)
(field_declaration name: (identifier) @property)
(field_initializer field: (identifier) @property)
(field_expression field: (identifier) @property)
(enum_variant name: (type_identifier) @constant)
(path variant: (type_identifier) @constant)
(match_arm pattern: (path variant: (type_identifier) @constant))
(identifier) @variable

(integer_literal) @number
(float_literal) @number.float
(boolean_literal) @boolean
(string_literal) @string
(character_literal) @character
(escape_sequence) @string.escape
(interpolation "{" @punctuation.special "}" @punctuation.special)
(format_specifier) @attribute
(comment) @comment

[
  "->"
  "=>"
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "+"
  "-"
  "*"
  "/"
  "%"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "::"
] @operator

[
  "("
  ")"
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

[
  ","
  ";"
  ":"
  "."
] @punctuation.delimiter
