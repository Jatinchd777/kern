pub mod token;

use crate::diagnostics::{Diagnostic, Diagnostics, Span};
pub use token::{Token, TokenKind};

#[derive(Debug, Clone)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

impl LexResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

pub fn lex(source: &str) -> LexResult {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    pos: usize,
    tokens: Vec<Token>,
    diagnostics: Diagnostics,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().collect(),
            pos: 0,
            tokens: Vec::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    fn run(mut self) -> LexResult {
        loop {
            self.skip_trivia();
            let Some((start, ch)) = self.peek() else {
                self.tokens.push(Token::new(TokenKind::Eof, self.here()));
                break;
            };

            match ch {
                'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(),
                '0'..='9' => self.lex_number(),
                '"' => self.lex_string(),
                '\'' => self.lex_char(),
                _ => {
                    self.bump();
                    match ch {
                        '+' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::PlusEq, start);
                            } else {
                                self.push_from(TokenKind::Plus, start);
                            }
                        }
                        '-' => {
                            if self.eat('>') {
                                self.push_from(TokenKind::Arrow, start);
                            } else if self.eat('=') {
                                self.push_from(TokenKind::MinusEq, start);
                            } else {
                                self.push_from(TokenKind::Minus, start);
                            }
                        }
                        '*' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::StarEq, start);
                            } else {
                                self.push_from(TokenKind::Star, start);
                            }
                        }
                        '/' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::SlashEq, start);
                            } else {
                                self.push_from(TokenKind::Slash, start);
                            }
                        }
                        '%' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::PercentEq, start);
                            } else {
                                self.push_from(TokenKind::Percent, start);
                            }
                        }
                        '=' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::EqEq, start);
                            } else if self.eat('>') {
                                self.push_from(TokenKind::FatArrow, start);
                            } else {
                                self.push_from(TokenKind::Eq, start);
                            }
                        }
                        '!' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::Ne, start);
                            } else {
                                self.push_from(TokenKind::Bang, start);
                            }
                        }
                        '<' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::Le, start);
                            } else if self.eat('<') {
                                self.push_from(TokenKind::Shl, start);
                            } else {
                                self.push_from(TokenKind::Lt, start);
                            }
                        }
                        '>' => {
                            if self.eat('=') {
                                self.push_from(TokenKind::Ge, start);
                            } else if self.eat('>') {
                                self.push_from(TokenKind::Shr, start);
                            } else {
                                self.push_from(TokenKind::Gt, start);
                            }
                        }
                        '&' => {
                            if self.eat('&') {
                                self.push_from(TokenKind::AndAnd, start);
                            } else {
                                self.push_from(TokenKind::Amp, start);
                            }
                        }
                        '|' => {
                            if self.eat('|') {
                                self.push_from(TokenKind::OrOr, start);
                            } else {
                                self.push_from(TokenKind::Pipe, start);
                            }
                        }
                        '^' => self.push_from(TokenKind::Caret, start),
                        ':' => {
                            if self.eat(':') {
                                self.push_from(TokenKind::ColonColon, start);
                            } else {
                                self.push_from(TokenKind::Colon, start);
                            }
                        }
                        '.' => self.push_from(TokenKind::Dot, start),
                        ',' => self.push_from(TokenKind::Comma, start),
                        ';' => self.push_from(TokenKind::Semi, start),
                        '(' => self.push_from(TokenKind::LParen, start),
                        ')' => self.push_from(TokenKind::RParen, start),
                        '{' => self.push_from(TokenKind::LBrace, start),
                        '}' => self.push_from(TokenKind::RBrace, start),
                        '[' => self.push_from(TokenKind::LBracket, start),
                        ']' => self.push_from(TokenKind::RBracket, start),
                        _ => {
                            self.diagnostics.error(
                                Span::new(start, start + ch.len_utf8()),
                                format!("unexpected character `{ch}`"),
                            );
                        }
                    }
                }
            }
        }

        LexResult {
            tokens: self.tokens,
            diagnostics: self.diagnostics.into_vec(),
        }
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.pos).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.peek().map(|(_, ch)| ch)
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).map(|(_, ch)| *ch)
    }

    fn bump(&mut self) -> Option<char> {
        let (_, ch) = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn here(&self) -> Span {
        let offset = self
            .peek()
            .map(|(offset, _)| offset)
            .unwrap_or(self.source.len());
        Span::new(offset, offset)
    }

    fn current_offset(&self) -> usize {
        self.peek()
            .map(|(offset, _)| offset)
            .unwrap_or(self.source.len())
    }

    fn push_from(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token::new(
            kind,
            Span::new(start, self.current_offset()),
        ));
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek_char() {
                Some(ch) if ch.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    self.bump();
                    self.bump();
                    while let Some(ch) = self.peek_char() {
                        if ch == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let start = self.current_offset();
                    self.bump();
                    self.bump();
                    let mut depth = 1usize;
                    while depth > 0 {
                        match (self.peek_char(), self.peek_at(1)) {
                            (Some('/'), Some('*')) => {
                                self.bump();
                                self.bump();
                                depth += 1;
                            }
                            (Some('*'), Some('/')) => {
                                self.bump();
                                self.bump();
                                depth -= 1;
                            }
                            (Some(_), _) => {
                                self.bump();
                            }
                            (None, _) => {
                                self.diagnostics.error(
                                    Span::new(start, self.source.len()),
                                    "unterminated block comment",
                                );
                                return;
                            }
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn lex_identifier(&mut self) {
        let start = self.current_offset();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.bump();
            } else {
                break;
            }
        }
        let text = &self.source[start..self.current_offset()];
        let kind = match text {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "loop" => TokenKind::Loop,
            "for" => TokenKind::For,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" => TokenKind::Return,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "use" => TokenKind::Use,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "print" => TokenKind::Print,
            _ => TokenKind::Ident(text.to_string()),
        };
        self.push_from(kind, start);
    }

    fn lex_number(&mut self) {
        let start = self.current_offset();

        // Radix-prefixed integers: 0x1f, 0b1010, 0o755.
        if self.peek_char() == Some('0') {
            if let Some(prefix) = self.peek_at(1) {
                let radix = match prefix {
                    'x' | 'X' => Some(16),
                    'b' | 'B' => Some(2),
                    'o' | 'O' => Some(8),
                    _ => None,
                };
                if let Some(radix) = radix {
                    self.bump();
                    self.bump();
                    let digits_start = self.current_offset();
                    while let Some(ch) = self.peek_char() {
                        if ch.is_digit(radix) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    let digits = &self.source[digits_start..self.current_offset()];
                    if digits.is_empty() {
                        self.diagnostics.error(
                            Span::new(start, self.current_offset()),
                            "expected digits after integer base prefix",
                        );
                    }
                    let value = u128::from_str_radix(digits, radix).unwrap_or_else(|_| {
                        self.diagnostics.error(
                            Span::new(start, self.current_offset()),
                            "integer literal is too large",
                        );
                        0
                    });
                    let suffix = self.lex_suffix();
                    self.push_from(
                        TokenKind::Int { value, suffix },
                        start,
                    );
                    return;
                }
            }
        }

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.bump();
            } else {
                break;
            }
        }

        let mut is_float = false;
        if self.peek_char() == Some('.') && self.peek_at(1).is_some_and(|ch| ch.is_ascii_digit())
        {
            is_float = true;
            self.bump();
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    self.bump();
                } else {
                    break;
                }
            }
        }

        if matches!(self.peek_char(), Some('e' | 'E')) {
            let next = self.peek_at(1);
            let next_next = self.peek_at(2);
            let exponent_starts = next.is_some_and(|ch| ch.is_ascii_digit())
                || (matches!(next, Some('+' | '-'))
                    && next_next.is_some_and(|ch| ch.is_ascii_digit()));
            if exponent_starts {
                is_float = true;
                self.bump();
                if matches!(self.peek_char(), Some('+' | '-')) {
                    self.bump();
                }
                while let Some(ch) = self.peek_char() {
                    if ch.is_ascii_digit() {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }

        let number_end = self.current_offset();
        let number_text = &self.source[start..number_end];
        let suffix = self.lex_suffix();

        if is_float || matches!(suffix.as_deref(), Some("f32" | "f64")) {
            let value = number_text.parse::<f64>().unwrap_or_else(|_| {
                self.diagnostics.error(
                    Span::new(start, number_end),
                    "invalid floating-point literal",
                );
                0.0
            });
            self.push_from(
                TokenKind::Float { value, suffix },
                start,
            );
        } else {
            let value = number_text.parse::<u128>().unwrap_or_else(|_| {
                self.diagnostics.error(
                    Span::new(start, number_end),
                    "integer literal is too large",
                );
                0
            });
            self.push_from(
                TokenKind::Int { value, suffix },
                start,
            );
        }
    }

    fn lex_suffix(&mut self) -> Option<String> {
        let start = self.current_offset();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.bump();
            } else {
                break;
            }
        }
        if self.current_offset() == start {
            None
        } else {
            Some(self.source[start..self.current_offset()].to_string())
        }
    }

    fn lex_string(&mut self) {
        let start = self.current_offset();
        self.bump(); // opening quote
        let mut value = String::new();
        let mut terminated = false;

        loop {
            match self.peek_char() {
                Some('"') => {
                    self.bump();
                    terminated = true;
                    break;
                }
                Some('\\') => {
                    self.bump();
                    if let Some(ch) = self.lex_escape() {
                        value.push(ch);
                    }
                }
                Some('\n') => {
                    self.diagnostics.error(
                        Span::new(start, self.current_offset()),
                        "unterminated string literal",
                    );
                    break;
                }
                Some(ch) => {
                    self.bump();
                    value.push(ch);
                }
                None => {
                    self.diagnostics.error(
                        Span::new(start, self.current_offset()),
                        "unterminated string literal",
                    );
                    break;
                }
            }
        }

        if !terminated {
            // Still produce the token so the parser can make progress.
        }
        self.push_from(TokenKind::Str(value), start);
    }

    fn lex_char(&mut self) {
        let start = self.current_offset();
        self.bump(); // opening quote

        let value = match self.peek_char() {
            None | Some('\n') => {
                self.diagnostics.error(
                    Span::new(start, self.current_offset()),
                    "unterminated character literal",
                );
                '\0'
            }
            Some('\\') => {
                self.bump();
                self.lex_escape().unwrap_or('\0')
            }
            Some(ch) => {
                self.bump();
                ch
            }
        };

        if !self.eat('\'') {
            let mut consumed = false;
            while let Some(ch) = self.peek_char() {
                if ch == '\'' {
                    self.bump();
                    consumed = true;
                    break;
                }
                if ch == '\n' {
                    break;
                }
                self.bump();
            }
            self.diagnostics.error(
                Span::new(start, self.current_offset()),
                if consumed {
                    "character literal must contain exactly one character"
                } else {
                    "unterminated character literal"
                },
            );
        }

        self.push_from(TokenKind::Char(value), start);
    }

    fn lex_escape(&mut self) -> Option<char> {
        let start = self.current_offset().saturating_sub(1);
        let ch = self.bump()?;
        let value = match ch {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '0' => '\0',
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            'x' => {
                let mut value = 0u32;
                for _ in 0..2 {
                    let Some(digit) = self.peek_char().and_then(|ch| ch.to_digit(16)) else {
                        self.diagnostics.error(
                            Span::new(start, self.current_offset()),
                            "invalid hexadecimal escape sequence",
                        );
                        return None;
                    };
                    self.bump();
                    value = value * 16 + digit;
                }
                char::from_u32(value).unwrap_or('\u{fffd}')
            }
            'u' => {
                if !self.eat('{') {
                    self.diagnostics.error(
                        Span::new(start, self.current_offset()),
                        "expected `{` after `\\u`",
                    );
                    return None;
                }
                let mut value = 0u32;
                let mut digits = 0;
                while let Some(ch) = self.peek_char() {
                    if ch == '}' {
                        break;
                    }
                    let Some(digit) = ch.to_digit(16) else {
                        self.diagnostics.error(
                            Span::new(start, self.current_offset()),
                            "invalid unicode escape sequence",
                        );
                        return None;
                    };
                    value = value.saturating_mul(16).saturating_add(digit);
                    digits += 1;
                    self.bump();
                }
                if !self.eat('}') || digits == 0 {
                    self.diagnostics.error(
                        Span::new(start, self.current_offset()),
                        "invalid unicode escape sequence",
                    );
                    return None;
                }
                char::from_u32(value).unwrap_or_else(|| {
                    self.diagnostics.error(
                        Span::new(start, self.current_offset()),
                        "invalid unicode scalar value",
                    );
                    '\u{fffd}'
                })
            }
            other => {
                self.diagnostics.error(
                    Span::new(start, self.current_offset()),
                    format!("unknown escape sequence `\\{other}`"),
                );
                other
            }
        };
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source).tokens.into_iter().map(|token| token.kind).collect()
    }

    #[test]
    fn lexes_keywords_operators_and_literals() {
        let tokens = kinds("fn main() -> i32 { let mut x: i32 = 0x2a; x += 1; return x; }");
        assert!(tokens.contains(&TokenKind::Fn));
        assert!(tokens.contains(&TokenKind::Arrow));
        assert!(tokens.contains(&TokenKind::PlusEq));
        assert!(tokens.contains(&TokenKind::Int {
            value: 42,
            suffix: None
        }));
    }

    #[test]
    fn skips_line_and_nested_block_comments() {
        let tokens = kinds("// line\n/* outer /* inner */ done */ let x = 1;");
        assert_eq!(
            tokens,
            vec![
                TokenKind::Let,
                TokenKind::Ident("x".into()),
                TokenKind::Eq,
                TokenKind::Int {
                    value: 1,
                    suffix: None
                },
                TokenKind::Semi,
                TokenKind::Eof
            ]
        );
    }

    #[test]
    fn lexes_escapes_and_suffixes() {
        let tokens = kinds(r#""a\n\x41\u{1F600}" 'z' 12u8 1.5f32"#);
        assert_eq!(tokens[0], TokenKind::Str("a\nA😀".into()));
        assert_eq!(tokens[1], TokenKind::Char('z'));
        assert_eq!(
            tokens[2],
            TokenKind::Int {
                value: 12,
                suffix: Some("u8".into())
            }
        );
        assert!(matches!(tokens[3], TokenKind::Float { value, .. } if (value - 1.5).abs() < f64::EPSILON));
    }

    #[test]
    fn reports_unterminated_string_without_panicking() {
        let result = lex("\"oops");
        assert!(result.has_errors());
        assert!(result.diagnostics[0].message.contains("unterminated"));
    }

    #[test]
    fn tracks_token_spans() {
        let result = lex("let x = 1;");
        assert_eq!(result.tokens[0].span, Span::new(0, 3));
        assert_eq!(result.tokens[1].span, Span::new(4, 5));
    }
}
