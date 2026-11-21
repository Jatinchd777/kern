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

(primitive_type) @type.builtin.kern
(type_identifier) @type.kern
(function_declaration name: (identifier) @function.kern)
(call_expression function: (identifier) @function.call.kern)
(parameter name: (identifier) @variable.parameter.kern)
(let_statement name: (identifier) @variable.kern)
(field_declaration name: (identifier) @property.kern)
(field_initializer field: (identifier) @property.kern)
(field_expression field: (identifier) @property.kern)
(enum_variant name: (type_identifier) @constant.kern)
(path variant: (type_identifier) @constant.kern)
(match_arm pattern: (path variant: (type_identifier) @constant.kern))
(expression (identifier) @variable.kern)

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
