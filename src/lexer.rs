#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Let,
    Print,
    Identifier(String),
    Integer(i64),
    Equals,
    Plus,
    Minus,
    Star,
    Slash,
    LeftParen,
    RightParen,
    Semicolon,

    EOF,
}
pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\n' | '\t' | '\r' => continue,

            '=' => tokens.push(Token::Equals),
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Star),
            '/' => tokens.push(Token::Slash),

            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            ';' => tokens.push(Token::Semicolon),

            '0'..='9' => {
                let mut number = ch.to_string();

                while let Some(next) = chars.peek() {
                    if next.is_ascii_digit() {
                        number.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                let value = number.parse::<i64>().unwrap();

                tokens.push(Token::Integer(value));
            }

            'a'..='z' | 'A'..='Z' | '_' => {
                let mut identifier = ch.to_string();

                while let Some(next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || *next == '_' {
                        identifier.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                let token = match identifier.as_str() {
                    "let" => Token::Let,
                    "print" => Token::Print,
                    _ => Token::Identifier(identifier),
                };

                tokens.push(token);
            }

            _ => panic!("unexpected character: {}", ch),
        }
    }

    tokens.push(Token::EOF);

    tokens
}
