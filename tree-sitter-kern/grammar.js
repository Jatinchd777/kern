const PREC = {
  assign: 1,
  or: 2,
  and: 3,
  compare: 4,
  bit_or: 5,
  bit_xor: 6,
  bit_and: 7,
  shift: 8,
  add: 9,
  multiply: 10,
  unary: 11,
  call: 12,
};

const commaSep = (rule) =>
  seq(rule, repeat(seq(",", rule)), optional(","));

module.exports = grammar({
  name: "kern",

  extras: ($) => [/\s/, $.comment],

  word: ($) => $._identifier,

  conflicts: ($) => [[ $.identifier, $.type_identifier ]],

  rules: {
    source_file: ($) => repeat($._item),

    _item: ($) =>
      choice(
        $.function_declaration,
        $.struct_declaration,
        $.enum_declaration,
        $.use_declaration,
      ),

    function_declaration: ($) =>
      seq(
        "fn",
        field("name", $.identifier),
        "(",
        optional(commaSep($.parameter)),
        ")",
        optional(seq("->", field("return_type", $._type))),
        field("body", $.block),
      ),

    parameter: ($) =>
      seq(field("name", $.identifier), ":", field("type", $._type)),

    struct_declaration: ($) =>
      seq(
        "struct",
        field("name", $.type_identifier),
        "{",
        optional(commaSep($.field_declaration)),
        "}",
      ),

    field_declaration: ($) =>
      seq(field("name", $.identifier), ":", field("type", $._type)),

    enum_declaration: ($) =>
      seq(
        "enum",
        field("name", $.type_identifier),
        "{",
        optional(commaSep($.enum_variant)),
        "}",
      ),

    enum_variant: ($) =>
      seq(
        field("name", $.type_identifier),
        optional(seq("(", commaSep($._type), ")")),
      ),

    use_declaration: ($) =>
      seq("use", $.path, ";"),

    block: ($) => seq("{", repeat($._statement), "}"),

    _statement: ($) =>
      choice(
        $.let_statement,
        $.assignment_statement,
        $.print_statement,
        $.cin_statement,
        $.if_statement,
        $.while_statement,
        $.loop_statement,
        $.for_statement,
        $.match_statement,
        $.return_statement,
        $.break_statement,
        $.continue_statement,
        $.expression_statement,
        $.block,
      ),

    let_statement: ($) =>
      seq(
        "let",
        optional("mut"),
        field("name", $.identifier),
        optional(seq(":", field("type", $._type))),
        optional(seq("=", field("value", $.expression))),
        ";",
      ),

    assignment_statement: ($) =>
      seq(
        field("target", $.expression),
        field("operator", choice("=", "+=", "-=", "*=", "/=", "%=")),
        field("value", $.expression),
        ";",
      ),

    print_statement: ($) =>
      seq(
        $.print_call,
        ";",
      ),

    print_call: ($) =>
      seq(
        field("function", choice("print", "println")),
        "(",
        optional(commaSep($.expression)),
        ")",
      ),

    cin_statement: ($) =>
      seq("cin", ">>", $.identifier, repeat(seq(">>", $.identifier)), ";"),

    if_statement: ($) =>
      seq(
        "if",
        field("condition", $.expression),
        field("consequence", $.block),
        optional(seq("else", field("alternative", choice($.block, $.if_statement)))),
      ),

    while_statement: ($) =>
      seq("while", field("condition", $.expression), field("body", $.block)),

    loop_statement: ($) => seq("loop", field("body", $.block)),

    for_statement: ($) =>
      seq(
        "for",
        "(",
        optional($._for_initializer),
        ";",
        optional($.expression),
        ";",
        optional($._for_update),
        ")",
        field("body", $.block),
      ),

    _for_initializer: ($) =>
      choice(
        seq(
          "let",
          optional("mut"),
          $.identifier,
          optional(seq(":", $._type)),
          optional(seq("=", $.expression)),
        ),
        seq($.expression, choice("=", "+=", "-=", "*=", "/=", "%="), $.expression),
      ),

    _for_update: ($) =>
      choice(
        seq($.expression, choice("=", "+=", "-=", "*=", "/=", "%="), $.expression),
        $.expression,
      ),

    match_statement: ($) =>
      seq("match", field("value", $.expression), "{", repeat($.match_arm), "}"),

    match_arm: ($) =>
      seq(
        field("pattern", choice($.path, $.wildcard_pattern)),
        "=>",
        field("body", choice($.block, $.print_call, $.expression)),
        optional(","),
      ),

    wildcard_pattern: ($) => "_",

    return_statement: ($) => seq("return", optional($.expression), ";"),
    break_statement: ($) => seq("break", ";"),
    continue_statement: ($) => seq("continue", ";"),
    expression_statement: ($) => seq($.expression, ";"),

    expression: ($) =>
      choice(
        $.identifier,
        $.path,
        $.literal,
        $.parenthesized_expression,
        $.call_expression,
        $.field_expression,
        $.struct_literal,
        $.index_expression,
        $.unary_expression,
        $.binary_expression,
      ),

    parenthesized_expression: ($) => seq("(", $.expression, ")"),

    call_expression: ($) =>
      prec(
        PREC.call,
        seq(
          field("function", $.identifier),
          "(",
          optional(commaSep($.expression)),
          ")",
        ),
      ),

    field_expression: ($) =>
      prec.left(PREC.call, seq($.expression, ".", field("field", $.identifier))),

    index_expression: ($) =>
      prec.left(PREC.call, seq($.expression, "[", $.expression, "]")),

    struct_literal: ($) =>
      seq(
        field("type", $.type_identifier),
        "{",
        optional(commaSep($.field_initializer)),
        "}",
      ),

    field_initializer: ($) =>
      seq(field("field", $.identifier), ":", field("value", $.expression)),

    path: ($) =>
      seq(
        field("type", $.type_identifier),
        "::",
        field("variant", $.type_identifier),
      ),

    unary_expression: ($) =>
      prec.right(
        PREC.unary,
        seq(field("operator", choice("-", "!")), $.expression),
      ),

    binary_expression: ($) =>
      choice(
        ...[
          ["||", PREC.or],
          ["&&", PREC.and],
          ["|", PREC.bit_or],
          ["^", PREC.bit_xor],
          ["&", PREC.bit_and],
          ["==", PREC.compare],
          ["!=", PREC.compare],
          ["<", PREC.compare],
          ["<=", PREC.compare],
          [">", PREC.compare],
          [">=", PREC.compare],
          ["<<", PREC.shift],
          [">>", PREC.shift],
          ["+", PREC.add],
          ["-", PREC.add],
          ["*", PREC.multiply],
          ["/", PREC.multiply],
          ["%", PREC.multiply],
        ].map(([operator, precedence]) =>
          prec.left(precedence, seq($.expression, operator, $.expression)),
        ),
      ),

    literal: ($) =>
      choice(
        $.integer_literal,
        $.float_literal,
        $.boolean_literal,
        $.character_literal,
        $.string_literal,
      ),

    integer_literal: (_) =>
      token(
        seq(
          choice(
            /0[xX][0-9a-fA-F](?:_?[0-9a-fA-F])*/,
            /0[bB][01](?:_?[01])*/,
            /0[oO][0-7](?:_?[0-7])*/,
            /[0-9](?:_?[0-9])*/,
          ),
          optional(/[iu](?:8|16|32|64|128|size)/),
        ),
      ),

    float_literal: (_) =>
      token(
        seq(
          choice(
            /[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*(?:[eE][+-]?[0-9](?:_?[0-9])*)?/,
            /[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*/,
          ),
          optional(/[fF](?:32|64)/),
        ),
      ),

    boolean_literal: (_) => choice("true", "false"),

    character_literal: ($) =>
      seq(
        "'",
        choice($.escape_sequence, token.immediate(/[^'\\\n]/)),
        "'",
      ),

    string_literal: ($) =>
      seq(
        '"',
        repeat(
          choice(
            $.escape_sequence,
            $.interpolation,
            token.immediate(/[^"\\{}\n]+/),
            token.immediate(/\{\{|\}\}/),
          ),
        ),
        '"',
      ),

    interpolation: ($) =>
      seq(
        "{",
        optional(choice($.format_specifier, $.expression)),
        "}",
      ),

    format_specifier: (_) =>
      seq(
        ":",
        choice(
          "x",
          "X",
          "b",
          "o",
          seq(".", /[0-9]+/),
          /0?[0-9]+/,
        ),
      ),

    escape_sequence: (_) =>
      token.immediate(
        seq(
          "\\",
          choice(
            /[nrt0\\"']/,
            seq("x", /[0-9a-fA-F]{2}/),
            seq("u{", /[0-9a-fA-F]+/, "}"),
          ),
        ),
      ),

    _type: ($) =>
      choice(
        $.primitive_type,
        $.type_identifier,
        $.generic_type,
        $.array_type,
        $.tuple_type,
        $.reference_type,
      ),

    primitive_type: (_) =>
      choice(
        "bool",
        "char",
        "String",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "isize",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "f32",
        "f64",
      ),

    generic_type: ($) =>
      seq($.type_identifier, "<", commaSep($._type), ">"),

    array_type: ($) =>
      seq("[", $._type, ";", $.integer_literal, "]"),

    tuple_type: ($) => seq("(", commaSep($._type), ")"),

    reference_type: ($) => seq("&", optional("mut"), $._type),

    type_identifier: ($) => $._identifier,
    identifier: ($) => $._identifier,
    _identifier: (_) => /[A-Za-z_][A-Za-z0-9_]*/,

    comment: (_) =>
      token(
        choice(
          seq("//", /[^\n]*/),
          /\/\*([^*]|\*+[^*/])*\*+\//,
        ),
      ),
  },
});

