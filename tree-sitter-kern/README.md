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

## Neovim colors

The LazyVim integration in `~/.config/nvim/lua/plugins/kern.lua` assigns
Kern-specific Tree-sitter highlight groups, reapplied whenever the colorscheme
changes:

| Syntax | Highlight group | Color |
| --- | --- | --- |
| Function declarations | `@function.kern` | Blue `#61afef` (bold) |
| Function calls | `@function.call.kern` | Blue `#61afef` |
| Variables, parameters, and fields | `@variable.kern`, `@variable.parameter.kern`, `@property.kern` | Blue `#61afef` |
| Kern types | `@type.kern`, `@type.builtin.kern` | Cyan `#56b6c2` |
| Enum variants | `@constant.kern` | Gold `#e5c07b` |

These are language-specific captures, so the palette does not change highlighting
in other filetypes. To customize it, edit the corresponding
`nvim_set_hl` calls in `kern.lua`; the capture names are defined in
`queries/highlights.scm`. Reopen the buffer or reload the colorscheme to apply
changes.
