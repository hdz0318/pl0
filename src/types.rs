use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Keywords
    Const,
    Var,
    Procedure,
    Program,
    Begin,
    End,
    If,
    Then,
    Else,
    While,
    Do,
    Call,
    Read,
    Write,
    Odd,
    // Operators
    Plus,
    Minus,
    Multiply,
    Divide,
    Equals,
    Hash,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
    Assignment,
    // Delimiters
    Comma,
    Semicolon,
    Period,
    LParen,
    RParen,
    // Literals and Identifiers
    Identifier(String),
    Number(i64),
    // Special
    Unknown,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OpCode {
    LIT,
    OPR,
    LOD,
    STO,
    CAL,
    INT,
    JMP,
    JPC,
    RED,
    WRT,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Instruction {
    pub f: OpCode,
    pub l: usize, // Level difference
    pub a: i64,   // Argument/Address
}

impl Instruction {
    pub fn new(f: OpCode, l: usize, a: i64) -> Self {
        Self { f, l, a }
    }
}

#[derive(Debug, Clone)]
pub enum SymbolType {
    Constant { val: i64 },
    Variable { level: usize, addr: i64 },
    Procedure { level: usize, addr: i64 },
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolType,
}
