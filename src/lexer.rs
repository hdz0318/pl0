use crate::types::TokenType;
use std::iter::Peekable;
use std::str::Chars;

pub struct Lexer<'a> {
    input: Peekable<Chars<'a>>,
    pub current_token: TokenType,
    pub line: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Self {
            input: input.chars().peekable(),
            current_token: TokenType::Unknown,
            line: 1,
        };
        lexer.next_token(); // Prime the first token
        lexer
    }

    pub fn next_token(&mut self) {
        self.skip_whitespace();

        if let Some(&c) = self.input.peek() {
            match c {
                'a'..='z' | 'A'..='Z' => self.scan_identifier_or_keyword(),
                '0'..='9' => self.scan_number(),
                '+' => {
                    self.input.next();
                    self.current_token = TokenType::Plus;
                }
                '-' => {
                    self.input.next();
                    self.current_token = TokenType::Minus;
                }
                '*' => {
                    self.input.next();
                    self.current_token = TokenType::Multiply;
                }
                '/' => {
                    self.input.next();
                    self.current_token = TokenType::Divide;
                }
                '=' => {
                    self.input.next();
                    self.current_token = TokenType::Equals;
                }
                '#' => {
                    self.input.next();
                    self.current_token = TokenType::Hash;
                }
                '<' => {
                    self.input.next();
                    if let Some(&'=') = self.input.peek() {
                        self.input.next();
                        self.current_token = TokenType::LessEqual;
                    } else if let Some(&'>') = self.input.peek() {
                        self.input.next();
                        self.current_token = TokenType::Hash; // Using Hash for <> (not equal)
                    } else {
                        self.current_token = TokenType::LessThan;
                    }
                }
                '>' => {
                    self.input.next();
                    if let Some(&'=') = self.input.peek() {
                        self.input.next();
                        self.current_token = TokenType::GreaterEqual;
                    } else {
                        self.current_token = TokenType::GreaterThan;
                    }
                }
                ':' => {
                    self.input.next();
                    if let Some(&'=') = self.input.peek() {
                        self.input.next();
                        self.current_token = TokenType::Assignment;
                    } else {
                        self.current_token = TokenType::Unknown; // Single ':' is not valid in PL/0
                    }
                }
                '(' => {
                    self.input.next();
                    self.current_token = TokenType::LParen;
                }
                ')' => {
                    self.input.next();
                    self.current_token = TokenType::RParen;
                }
                ',' => {
                    self.input.next();
                    self.current_token = TokenType::Comma;
                }
                ';' => {
                    self.input.next();
                    self.current_token = TokenType::Semicolon;
                }
                '.' => {
                    self.input.next();
                    self.current_token = TokenType::Period;
                }
                _ => {
                    self.input.next();
                    self.current_token = TokenType::Unknown;
                }
            }
        } else {
            self.current_token = TokenType::Eof;
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.input.peek() {
            if c.is_whitespace() {
                if c == '\n' {
                    self.line += 1;
                }
                self.input.next();
            } else {
                break;
            }
        }
    }

    fn scan_identifier_or_keyword(&mut self) {
        let mut ident = String::new();
        while let Some(&c) = self.input.peek() {
            if c.is_alphanumeric() {
                ident.push(c);
                self.input.next();
            } else {
                break;
            }
        }

        self.current_token = match ident.as_str() {
            "program" => TokenType::Program,
            "const" => TokenType::Const,
            "var" => TokenType::Var,
            "procedure" => TokenType::Procedure,
            "begin" => TokenType::Begin,
            "end" => TokenType::End,
            "if" => TokenType::If,
            "then" => TokenType::Then,
            "else" => TokenType::Else,
            "while" => TokenType::While,
            "do" => TokenType::Do,
            "call" => TokenType::Call,
            "read" => TokenType::Read,
            "write" => TokenType::Write,
            "odd" => TokenType::Odd,
            _ => TokenType::Identifier(ident),
        };
    }

    fn scan_number(&mut self) {
        let mut num_str = String::new();
        while let Some(&c) = self.input.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                self.input.next();
            } else {
                break;
            }
        }
        if let Ok(num) = num_str.parse::<i64>() {
            self.current_token = TokenType::Number(num);
        } else {
            self.current_token = TokenType::Unknown; // Overflow or error
        }
    }
}
