(block) @fold
(function_declaration body: (block) @fold)
(struct_declaration) @fold
(enum_declaration) @fold
(match_statement) @fold
(if_statement consequence: (block) @fold)
(while_statement body: (block) @fold)
(loop_statement body: (block) @fold)
(for_statement body: (block) @fold)
