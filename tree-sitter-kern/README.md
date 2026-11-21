# tree-sitter-kern

Tree-sitter grammar and Neovim queries for Kern (`*.kr`). The grammar follows
the Kern compiler's current lexer/parser: functions, types, declarations,
control flow, enums, structs, match statements, imports, `cin`, and the
`print`/`println` interpolation and format-string syntax are represented.

The compiler currently parses `use` declarations but reports them as
unsupported semantically. Enum payloads, arrays, and generic type syntax are
also accepted by portions of the parser but are not generally usable in
compiled programs yet; the grammar reflects syntax rather than promising
semantic support.

## Generate and test

Install `tree-sitter-cli` 0.26.1 or later and a C compiler, then run:

```sh
tree-sitter generate
tree-sitter test
tree-sitter parse ../examples/control_flow.kr
```

The generated `src/parser.c` is checked in so editor installation does not need
to generate it. For Neovim, register this directory as a local parser source;
see the parent Kern README and the LazyVim configuration under
`~/.config/nvim/lua/plugins/kern.lua`.

## Syntax coverage

`queries/` contains highlight, locals, indentation, and folding queries. Strings
support `{expression}` interpolation, `{}` positional slots, `{:x}`, `{:X}`,
`{:b}`, `{:o}`, `{:.N}`, `{:N}`, `{:0N}`, and doubled braces.
