use crate::types::Operator;

#[derive(Debug, Clone)]
pub struct Program {
    pub block: Block,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub consts: Vec<ConstDecl>,
    pub vars: Vec<String>,
    pub procedures: Vec<ProcedureDecl>,
    pub statement: Statement,
    pub scope_id: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub value: i64,
}

#[derive(Debug, Clone)]
pub struct ProcedureDecl {
    pub name: String,
    pub params: Vec<String>,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assignment {
        name: String,
        expr: Expr,
        line: usize,
    },
    Call {
        name: String,
        args: Vec<Expr>,
        line: usize,
    },
    BeginEnd {
        statements: Vec<Statement>,
    },
    If {
        condition: Condition,
        then_stmt: Box<Statement>,
        else_stmt: Option<Box<Statement>>,
        line: usize,
    },
    While {
        condition: Condition,
        body: Box<Statement>,
        line: usize,
    },
    Read {
        names: Vec<String>,
        line: usize,
    },
    Write {
        exprs: Vec<Expr>,
        line: usize,
    },
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Condition {
    Odd {
        expr: Expr,
    },
    Compare {
        left: Expr,
        op: Operator,
        right: Expr,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Binary {
        left: Box<Expr>,
        op: Operator,
        right: Box<Expr>,
    },
    Unary {
        op: Operator,
        expr: Box<Expr>,
    }, // For unary minus
    Number(i64),
    Identifier(String),
}
