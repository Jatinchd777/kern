(function_declaration
  body: (block) @local.scope)

(block) @local.scope

(parameter
  name: (identifier) @local.definition)

(let_statement
  name: (identifier) @local.definition)

(identifier) @local.reference
