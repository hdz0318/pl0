use crate::ast::*;
use crate::lexer::Lexer;
use crate::types::{Operator, TokenType};

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    pub errors: Vec<ParseError>,
    verbose: bool,
}

type ParseResult<T> = Result<T, ()>;

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>, verbose: bool) -> Self {
        Self {
            lexer,
            errors: Vec::new(),
            verbose,
        }
    }

    fn error(&mut self, msg: &str) -> ParseResult<()> {
        self.errors.push(ParseError {
            line: self.lexer.token_line,
            col: self.lexer.token_col,
            message: msg.to_string(),
        });
        Err(())
    }

    fn next(&mut self) {
        if self.verbose {
            println!("Token: {:?}", self.lexer.current_token);
        }
        self.lexer.next_token();
    }

    fn expect(&mut self, token: TokenType) -> ParseResult<()> {
        if self.lexer.current_token == token {
            self.next();
            Ok(())
        } else {
            let msg = format!("Expected {:?}, found {:?}", token, self.lexer.current_token);
            self.error(&msg)
        }
    }

    pub fn parse(&mut self) -> ParseResult<Program> {
        self.program()
    }

    fn program(&mut self) -> ParseResult<Program> {
        if self.lexer.current_token == TokenType::Program {
            self.next();
            if let TokenType::Identifier(_) = self.lexer.current_token {
                self.next();
            } else {
                self.error("Expected program name")?;
                return Err(());
            }
            self.expect(TokenType::Semicolon)?;
        } else {
            self.error("Expected 'program'")?;
            return Err(());
        }

        let block = self.block()?;
        Ok(Program { block })
    }

    fn block(&mut self) -> ParseResult<Block> {
        let mut consts = Vec::new();
        let mut vars = Vec::new();
        let mut procedures = Vec::new();

        if self.lexer.current_token == TokenType::Const {
            consts = self.const_decl()?;
        }

        if self.lexer.current_token == TokenType::Var {
            vars = self.var_decl()?;
        }

        while self.lexer.current_token == TokenType::Procedure {
            procedures.push(self.proc_decl()?);
        }

        let statement = self.statement()?;

        Ok(Block {
            consts,
            vars,
            procedures,
            statement,
            scope_id: None,
        })
    }

    fn const_decl(&mut self) -> ParseResult<Vec<ConstDecl>> {
        let mut consts = Vec::new();
        self.next(); // consume 'const'
        loop {
            if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                self.next();
                if self.lexer.current_token == TokenType::Assignment
                    || self.lexer.current_token == TokenType::Equals
                {
                    self.next();
                } else {
                    self.error("Expected :=")?;
                    return Err(());
                }

                if let TokenType::Number(val) = self.lexer.current_token {
                    consts.push(ConstDecl { name, value: val });
                    self.next();
                } else {
                    self.error("Expected number")?;
                    return Err(());
                }
            } else {
                self.error("Expected identifier")?;
                return Err(());
            }

            if self.lexer.current_token == TokenType::Comma {
                self.next();
            } else {
                break;
            }
        }
        self.expect(TokenType::Semicolon)?;
        Ok(consts)
    }

    fn var_decl(&mut self) -> ParseResult<Vec<String>> {
        let mut vars = Vec::new();
        self.next(); // consume 'var'
        loop {
            if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                vars.push(name);
                self.next();
            } else {
                self.error("Expected identifier")?;
                return Err(());
            }

            if self.lexer.current_token == TokenType::Comma {
                self.next();
            } else {
                break;
            }
        }
        self.expect(TokenType::Semicolon)?;
        Ok(vars)
    }

    fn proc_decl(&mut self) -> ParseResult<ProcedureDecl> {
        self.next(); // consume 'procedure'
        let name = if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
            self.next();
            name
        } else {
            self.error("Expected identifier")?;
            return Err(());
        };

        let mut params = Vec::new();
        if self.lexer.current_token == TokenType::LParen {
            self.next();
            loop {
                if let TokenType::Identifier(param_name) = self.lexer.current_token.clone() {
                    params.push(param_name);
                    self.next();
                } else {
                    self.error("Expected parameter name")?;
                    return Err(());
                }

                if self.lexer.current_token == TokenType::Comma {
                    self.next();
                } else {
                    break;
                }
            }
            self.expect(TokenType::RParen)?;
        }

        self.expect(TokenType::Semicolon)?;
        let block = self.block()?;
        self.expect(TokenType::Semicolon)?;

        Ok(ProcedureDecl {
            name,
            params,
            block,
        })
    }

    fn is_start_of_statement(&self) -> bool {
        matches!(
            self.lexer.current_token,
            TokenType::Identifier(_)
                | TokenType::Call
                | TokenType::Begin
                | TokenType::If
                | TokenType::While
                | TokenType::Read
                | TokenType::Write
        )
    }

    fn synchronize(&mut self) {
        while self.lexer.current_token != TokenType::Eof {
            if self.lexer.current_token == TokenType::Semicolon {
                return;
            }
            if self.is_start_of_statement() {
                return;
            }
            if self.lexer.current_token == TokenType::End {
                return;
            }
            self.next();
        }
    }

    fn statement(&mut self) -> ParseResult<Statement> {
        match self.lexer.current_token.clone() {
            TokenType::Identifier(name) => {
                self.next();
                if self.lexer.current_token == TokenType::Assignment {
                    self.next();
                    let expr = self.expression()?;
                    Ok(Statement::Assignment { name, expr })
                } else {
                    self.error("Expected :=")?;
                    Err(())
                }
            }
            TokenType::Call => {
                self.next();
                if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                    self.next();
                    let mut args = Vec::new();
                    if self.lexer.current_token == TokenType::LParen {
                        self.next();
                        loop {
                            args.push(self.expression()?);
                            if self.lexer.current_token == TokenType::Comma {
                                self.next();
                            } else {
                                break;
                            }
                        }
                        self.expect(TokenType::RParen)?;
                    }
                    Ok(Statement::Call { name, args })
                } else {
                    self.error("Expected identifier")?;
                    Err(())
                }
            }
            TokenType::Begin => {
                self.next();
                let mut statements = Vec::new();

                loop {
                    if self.lexer.current_token == TokenType::End {
                        break;
                    }
                    if self.lexer.current_token == TokenType::Eof {
                        self.error("Expected 'end'")?;
                        return Err(());
                    }

                    match self.statement() {
                        Ok(stmt) => {
                            if !matches!(stmt, Statement::Empty) {
                                statements.push(stmt);
                            } else {
                                if self.lexer.current_token != TokenType::Semicolon
                                    && self.lexer.current_token != TokenType::End
                                {
                                    self.errors.push(ParseError {
                                        line: self.lexer.token_line,
                                        col: self.lexer.token_col,
                                        message: format!(
                                            "Unexpected token: {:?}",
                                            self.lexer.current_token
                                        ),
                                    });
                                    self.synchronize();
                                }
                            }
                        }
                        Err(_) => {
                            self.synchronize();
                        }
                    }

                    if self.lexer.current_token == TokenType::Semicolon {
                        self.next();
                    } else if self.lexer.current_token == TokenType::End {
                        break;
                    } else {
                        if self.is_start_of_statement() {
                            self.errors.push(ParseError {
                                line: self.lexer.token_line,
                                col: self.lexer.token_col,
                                message: "Expected ';'".to_string(),
                            });
                        } else if self.lexer.current_token != TokenType::Eof {
                            // If we haven't already synchronized (which we would have if statement was Empty and invalid)
                            // We might be here if statement was valid but followed by garbage.
                            self.errors.push(ParseError {
                                line: self.lexer.token_line,
                                col: self.lexer.token_col,
                                message: format!(
                                    "Unexpected token: {:?}",
                                    self.lexer.current_token
                                ),
                            });
                            self.synchronize();
                        }
                    }
                }
                self.expect(TokenType::End)?;
                Ok(Statement::BeginEnd { statements })
            }
            TokenType::If => {
                self.next();
                let condition = self.condition()?;
                self.expect(TokenType::Then)?;
                let then_stmt = Box::new(self.statement()?);
                let else_stmt = if self.lexer.current_token == TokenType::Else {
                    self.next();
                    Some(Box::new(self.statement()?))
                } else {
                    None
                };
                Ok(Statement::If {
                    condition,
                    then_stmt,
                    else_stmt,
                })
            }
            TokenType::While => {
                self.next();
                let condition = self.condition()?;
                self.expect(TokenType::Do)?;
                let body = Box::new(self.statement()?);
                Ok(Statement::While { condition, body })
            }
            TokenType::Read => {
                self.next();
                let mut names = Vec::new();
                if self.lexer.current_token == TokenType::LParen {
                    self.next();
                    loop {
                        if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                            names.push(name);
                            self.next();
                        } else {
                            self.error("Expected identifier")?;
                            return Err(());
                        }
                        if self.lexer.current_token == TokenType::Comma {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    self.expect(TokenType::RParen)?;
                } else {
                    if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                        names.push(name);
                        self.next();
                    } else {
                        self.error("Expected identifier or '('")?;
                        return Err(());
                    }
                }
                Ok(Statement::Read { names })
            }
            TokenType::Write => {
                self.next();
                let mut exprs = Vec::new();
                if self.lexer.current_token == TokenType::LParen {
                    self.next();
                    loop {
                        exprs.push(self.expression()?);
                        if self.lexer.current_token == TokenType::Comma {
                            self.next();
                        } else {
                            break;
                        }
                    }
                    self.expect(TokenType::RParen)?;
                } else {
                    exprs.push(self.expression()?);
                }
                Ok(Statement::Write { exprs })
            }
            _ => Ok(Statement::Empty),
        }
    }

    fn condition(&mut self) -> ParseResult<Condition> {
        if self.lexer.current_token == TokenType::Odd {
            self.next();
            let expr = self.expression()?;
            Ok(Condition::Odd { expr })
        } else {
            let left = self.expression()?;
            let op = match self.lexer.current_token {
                TokenType::Equals => Operator::EQL,
                TokenType::Hash => Operator::NEQ,
                TokenType::LessThan => Operator::LSS,
                TokenType::LessEqual => Operator::LEQ,
                TokenType::GreaterThan => Operator::GTR,
                TokenType::GreaterEqual => Operator::GEQ,
                _ => {
                    self.error("Expected comparison operator")?;
                    return Err(());
                }
            };
            self.next();
            let right = self.expression()?;
            Ok(Condition::Compare { left, op, right })
        }
    }

    fn expression(&mut self) -> ParseResult<Expr> {
        let mut expr = if self.lexer.current_token == TokenType::Plus {
            self.next();
            self.term()?
        } else if self.lexer.current_token == TokenType::Minus {
            self.next();
            Expr::Unary {
                op: Operator::NEG,
                expr: Box::new(self.term()?),
            }
        } else {
            self.term()?
        };

        loop {
            match self.lexer.current_token {
                TokenType::Plus => {
                    self.next();
                    let right = self.term()?;
                    expr = Expr::Binary {
                        left: Box::new(expr),
                        op: Operator::ADD,
                        right: Box::new(right),
                    };
                }
                TokenType::Minus => {
                    self.next();
                    let right = self.term()?;
                    expr = Expr::Binary {
                        left: Box::new(expr),
                        op: Operator::SUB,
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn term(&mut self) -> ParseResult<Expr> {
        let mut expr = self.factor()?;
        loop {
            match self.lexer.current_token {
                TokenType::Multiply => {
                    self.next();
                    let right = self.factor()?;
                    expr = Expr::Binary {
                        left: Box::new(expr),
                        op: Operator::MUL,
                        right: Box::new(right),
                    };
                }
                TokenType::Divide => {
                    self.next();
                    let right = self.factor()?;
                    expr = Expr::Binary {
                        left: Box::new(expr),
                        op: Operator::DIV,
                        right: Box::new(right),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn factor(&mut self) -> ParseResult<Expr> {
        match self.lexer.current_token.clone() {
            TokenType::Identifier(name) => {
                self.next();
                Ok(Expr::Identifier(name))
            }
            TokenType::Number(val) => {
                self.next();
                Ok(Expr::Number(val))
            }
            TokenType::LParen => {
                self.next();
                let expr = self.expression()?;
                self.expect(TokenType::RParen)?;
                Ok(expr)
            }
            _ => {
                self.error("Expected identifier, number, or '('")?;
                Err(())
            }
        }
    }
}
