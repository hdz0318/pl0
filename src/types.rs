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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i64)]
pub enum Operator {
    RET = 0,
    NEG = 1,
    ADD = 2,
    SUB = 3,
    MUL = 4,
    DIV = 5,
    ODD = 6,
    EQL = 8,
    NEQ = 9,
    LSS = 10,
    GEQ = 11,
    GTR = 12,
    LEQ = 13,
    WRT = 14,
    WRL = 15,
    RED = 16,
}

impl Operator {
    pub fn from_i64(val: i64) -> Option<Self> {
        match val {
            0 => Some(Operator::RET),
            1 => Some(Operator::NEG),
            2 => Some(Operator::ADD),
            3 => Some(Operator::SUB),
            4 => Some(Operator::MUL),
            5 => Some(Operator::DIV),
            6 => Some(Operator::ODD),
            8 => Some(Operator::EQL),
            9 => Some(Operator::NEQ),
            10 => Some(Operator::LSS),
            11 => Some(Operator::GEQ),
            12 => Some(Operator::GTR),
            13 => Some(Operator::LEQ),
            14 => Some(Operator::WRT),
            15 => Some(Operator::WRL),
            16 => Some(Operator::RED),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
