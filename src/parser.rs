use crate::lexer::Lexer;
use crate::symbol_table::SymbolTable;
use crate::types::{Instruction, OpCode, Operator, Symbol, SymbolType, TokenType};

pub struct CodeGenerator {
    pub code: Vec<Instruction>,
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self { code: Vec::new() }
    }

    pub fn emit(&mut self, f: OpCode, l: usize, a: i64) {
        self.code.push(Instruction::new(f, l, a));
    }

    pub fn next_address(&self) -> usize {
        self.code.len()
    }

    pub fn backpatch(&mut self, addr: usize, val: i64) {
        if addr < self.code.len() {
            self.code[addr].a = val;
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    pub generator: CodeGenerator,
    symbol_table: SymbolTable,
    level: usize, // Current nesting level
    pub errors: Vec<ParseError>,
}

type ParseResult = Result<(), ()>;

impl<'a> Parser<'a> {
    pub fn new(lexer: Lexer<'a>) -> Self {
        Self {
            lexer,
            generator: CodeGenerator::new(),
            symbol_table: SymbolTable::new(),
            level: 0,
            errors: Vec::new(),
        }
    }

    fn error(&mut self, msg: &str) -> ParseResult {
        self.errors.push(ParseError {
            line: self.lexer.token_line,
            col: self.lexer.token_col,
            message: msg.to_string(),
        });
        Err(())
    }

    fn error_at(&mut self, line: usize, col: usize, msg: &str) -> ParseResult {
        self.errors.push(ParseError {
            line,
            col,
            message: msg.to_string(),
        });
        Err(())
    }

    fn synchronize(&mut self) {
        while self.lexer.current_token != TokenType::Eof {
            if self.lexer.current_token == TokenType::Semicolon {
                return;
            }
            match self.lexer.current_token {
                TokenType::Const
                | TokenType::Var
                | TokenType::Procedure
                | TokenType::Begin
                | TokenType::If
                | TokenType::While
                | TokenType::Call
                | TokenType::Read
                | TokenType::Write
                | TokenType::End => return,
                _ => self.next(),
            }
        }
    }

    fn next(&mut self) {
        self.lexer.next_token();
    }

    fn expect(&mut self, token: TokenType) -> ParseResult {
        if self.lexer.current_token == token {
            self.next();
            Ok(())
        } else {
            self.error(&format!(
                "Expected {:?}, found {:?}",
                token, self.lexer.current_token
            ))
        }
    }

    fn emit(&mut self, f: OpCode, l: usize, a: i64) {
        self.generator.emit(f, l, a);
    }

    fn enter(&mut self, name: String, kind: SymbolType) -> ParseResult {
        if let Err(msg) = self.symbol_table.define(Symbol { name, kind }) {
            self.error(&msg)
        } else {
            Ok(())
        }
    }

    fn position(&self, name: &str) -> Option<(usize, &Symbol)> {
        if let Some(sym) = self.symbol_table.resolve(name) {
            // We need to return (level, symbol).
            // But `position` originally returned (index, symbol).
            // The caller uses `index`? No, let's check usage.
            // Usage: `let (level, addr) = if let Some((_, sym)) = self.position(&name) { ... }`
            // The caller ignores the index!
            // Wait, let's check `statement` and `factor`.
            // `let (level, addr) = if let Some((_, sym)) = self.position(&name)`
            // Yes, the first element of the tuple is ignored.
            // So we can just return `(0, sym)` or change the signature.
            // Let's change the signature to return `Option<&Symbol>`.
            // But wait, I need to check all call sites.
            Some((0, sym))
        } else {
            None
        }
    }

    pub fn parse(&mut self) -> ParseResult {
        self.program()
    }

    // <prog> → program <id>；<block>
    fn program(&mut self) -> ParseResult {
        if self.lexer.current_token == TokenType::Program {
            self.next();
            if let TokenType::Identifier(_) = self.lexer.current_token {
                self.next();
            } else {
                self.error("Expected program name")?;
            }
            self.expect(TokenType::Semicolon)?;
        } else {
            self.error("Expected 'program'")?;
        }

        self.block()
    }

    // <block> → [<condecl>][<vardecl>][<proc>]<body>
    fn block(&mut self) -> ParseResult {
        // let tx0 = self.table.len(); // Save symbol table index (No longer needed with Scope Stack)
        let jmp_addr = self.generator.next_address();
        self.emit(OpCode::JMP, 0, 0); // Jump to start of body (placeholder)

        // We need to allocate space for SL, DL, RA (3 units)
        // But the exact amount of space for variables is known only after parsing declarations.
        // So we emit INT later? No, INT is executed at runtime.
        // The JMP jumps over the procedure declarations.

        // Wait, the standard PL/0 structure is:
        // JMP 0, 0  (Jump to main body)
        // ... procedures ...
        // Main body starts here.
        // INT 0, vars (Allocate vars for main)

        // But for nested procedures?
        // Procedure code is generated inline.
        // When a procedure is called, we jump to it.

        // Correct logic for Block:
        // 1. Reserve space for JMP to body (if we have procedures).
        //    Actually, standard PL/0 generates code for procedures *before* the body of the current block.
        //    So the current block's code starts *after* its procedures.
        //    So we need a JMP to skip the procedures?
        //    Yes.

        // Let's follow the structure:
        // Declarations
        // Statement (Body)

        let mut data_alloc_size = 3; // SL, DL, RA

        // Const Declaration
        if self.lexer.current_token == TokenType::Const {
            self.const_decl()?;
        }

        // Var Declaration
        if self.lexer.current_token == TokenType::Var {
            data_alloc_size += self.var_decl()?;
        }

        // Procedure Declaration
        if self.lexer.current_token == TokenType::Procedure {
            self.proc_decl()?;
        }

        // Fix the JMP to point to the start of the body
        self.generator
            .backpatch(jmp_addr, self.generator.next_address() as i64);

        // Allocate space
        self.emit(OpCode::INT, 0, data_alloc_size as i64);

        if let Err(_) = self.statement() {
            self.synchronize();
        }

        self.emit(OpCode::OPR, 0, Operator::RET as i64); // Return

        // Restore symbol table (No longer needed with Scope Stack, handled by exit_scope in proc_decl)
        // self.table.truncate(tx0);
        Ok(())
    }

    // <condecl> → const <const>{,<const>}
    // <const> → <id>:=<integer>
    fn const_decl(&mut self) -> ParseResult {
        self.next(); // consume 'const'
        loop {
            if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                self.next();
                if self.lexer.current_token == TokenType::Assignment {
                    self.next();
                } else {
                    self.error("Expected :=")?;
                }

                if let TokenType::Number(val) = self.lexer.current_token {
                    self.enter(name, SymbolType::Constant { val })?;
                    self.next();
                } else {
                    self.error("Expected number")?;
                }
            } else {
                self.error("Expected identifier")?;
            }

            if self.lexer.current_token == TokenType::Comma {
                self.next();
            } else {
                break;
            }
        }
        self.expect(TokenType::Semicolon)
    }

    // <vardecl> → var <id>{,<id>}
    fn var_decl(&mut self) -> Result<usize, ()> {
        let mut vars = 0;
        self.next(); // consume 'var'
        loop {
            if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                self.enter(
                    name,
                    SymbolType::Variable {
                        level: self.level,
                        addr: 3 + vars as i64,
                    },
                )?;
                vars += 1;
                self.next();
            } else {
                self.error("Expected identifier")?;
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

    // <proc> → procedure <id>（[<id>{,<id>}]）;<block>{;<proc>}
    fn proc_decl(&mut self) -> ParseResult {
        while self.lexer.current_token == TokenType::Procedure {
            self.next();
            let name = if let TokenType::Identifier(n) = self.lexer.current_token.clone() {
                n
            } else {
                self.error("Expected procedure name")?;
                String::new()
            };
            self.next();

            self.enter(
                name,
                SymbolType::Procedure {
                    level: self.level,
                    addr: self.generator.next_address() as i64,
                },
            )?;

            self.level += 1;
            self.symbol_table.enter_scope(); // Enter scope for parameters and body

            let mut params = Vec::new();
            if self.lexer.current_token == TokenType::LParen {
                self.next();
                if self.lexer.current_token != TokenType::RParen {
                    loop {
                        if let TokenType::Identifier(pname) = self.lexer.current_token.clone() {
                            params.push(pname);
                            self.next();
                        } else {
                            self.error("Expected parameter name")?;
                        }

                        if self.lexer.current_token == TokenType::Comma {
                            self.next();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(TokenType::RParen)?;
            }

            self.expect(TokenType::Semicolon)?;

            let count = params.len();
            for (i, pname) in params.into_iter().enumerate() {
                let addr = -((count - i) as i64);
                self.enter(
                    pname,
                    SymbolType::Variable {
                        level: self.level,
                        addr,
                    },
                )?;
            }

            self.block()?;

            self.expect(TokenType::Semicolon)?;
            self.symbol_table.exit_scope(); // Exit scope for parameters and body
            self.level -= 1;
        }
        Ok(())
    }

    // <body> → begin <statement>{;<statement>}end
    // <statement> → ...
    fn statement(&mut self) -> ParseResult {
        match self.lexer.current_token.clone() {
            TokenType::Identifier(name) => {
                // Assignment: <id> := <exp>
                // Find symbol
                let line = self.lexer.token_line;
                let col = self.lexer.token_col;
                self.next();
                let (level, addr) = if let Some((_, sym)) = self.position(&name) {
                    match sym.kind {
                        SymbolType::Variable { level, addr } => (level, addr),
                        _ => {
                            self.error_at(line, col, "Assignment to non-variable")?;
                            (0, 0)
                        }
                    }
                } else {
                    self.error_at(line, col, &format!("Undefined variable: {}", name))?;
                    (0, 0)
                };

                self.expect(TokenType::Assignment)?;
                self.expression()?;
                self.emit(OpCode::STO, self.level - level, addr);
            }
            TokenType::Call => {
                self.next();
                if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                    let line = self.lexer.token_line;
                    let col = self.lexer.token_col;
                    self.next();
                    let (level, addr) = if let Some((_, sym)) = self.position(&name) {
                        match sym.kind {
                            SymbolType::Procedure { level, addr } => (level, addr),
                            _ => {
                                self.error_at(line, col, "Call to non-procedure")?;
                                (0, 0)
                            }
                        }
                    } else {
                        self.error_at(line, col, &format!("Undefined procedure: {}", name))?;
                        (0, 0)
                    };

                    // Handle parameters: call <id>[（<exp>{,<exp>}）]
                    if self.lexer.current_token == TokenType::LParen {
                        self.next();
                        let mut args_count = 0;
                        loop {
                            self.expression()?;
                            args_count += 1;
                            if self.lexer.current_token == TokenType::Comma {
                                self.next();
                            } else {
                                break;
                            }
                        }
                        self.expect(TokenType::RParen)?;

                        self.emit(OpCode::CAL, self.level - level, addr);
                        self.emit(OpCode::INT, 0, -(args_count as i64));
                    } else {
                        self.emit(OpCode::CAL, self.level - level, addr);
                    }
                } else {
                    self.error("Expected identifier after call")?;
                }
            }
            TokenType::Begin => {
                self.next();
                if let Err(_) = self.statement() {
                    self.synchronize();
                }
                while self.lexer.current_token == TokenType::Semicolon {
                    self.next();
                    if let Err(_) = self.statement() {
                        self.synchronize();
                    }
                }
                self.expect(TokenType::End)?;
            }
            TokenType::If => {
                self.next();
                self.condition()?;
                self.expect(TokenType::Then)?;
                let jpc_idx = self.generator.next_address();
                self.emit(OpCode::JPC, 0, 0);
                self.statement()?;
                if self.lexer.current_token == TokenType::Else {
                    self.next();
                    let jmp_idx = self.generator.next_address();
                    self.emit(OpCode::JMP, 0, 0);
                    self.generator
                        .backpatch(jpc_idx, self.generator.next_address() as i64);
                    self.statement()?;
                    self.generator
                        .backpatch(jmp_idx, self.generator.next_address() as i64);
                } else {
                    self.generator
                        .backpatch(jpc_idx, self.generator.next_address() as i64);
                }
            }
            TokenType::While => {
                self.next();
                let start_idx = self.generator.next_address();
                self.condition()?;
                self.expect(TokenType::Do)?;
                let jpc_idx = self.generator.next_address();
                self.emit(OpCode::JPC, 0, 0);
                self.statement()?;
                self.emit(OpCode::JMP, 0, start_idx as i64);
                self.generator
                    .backpatch(jpc_idx, self.generator.next_address() as i64);
            }
            TokenType::Read => {
                self.next();
                self.expect(TokenType::LParen)?;
                loop {
                    if let TokenType::Identifier(name) = self.lexer.current_token.clone() {
                        let line = self.lexer.token_line;
                        let col = self.lexer.token_col;
                        self.next();
                        let (level, addr) = if let Some((_, sym)) = self.position(&name) {
                            match sym.kind {
                                SymbolType::Variable { level, addr } => (level, addr),
                                _ => {
                                    self.error_at(line, col, "Read to non-variable")?;
                                    (0, 0)
                                }
                            }
                        } else {
                            self.error_at(line, col, "Undefined variable")?;
                            (0, 0)
                        };
                        self.emit(OpCode::RED, self.level - level, addr);
                    } else {
                        self.error("Expected identifier")?;
                    }

                    if self.lexer.current_token == TokenType::Comma {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(TokenType::RParen)?;
            }
            TokenType::Write => {
                self.next();
                self.expect(TokenType::LParen)?;
                loop {
                    self.expression()?;
                    self.emit(OpCode::WRT, 0, 0);
                    if self.lexer.current_token == TokenType::Comma {
                        self.next();
                    } else {
                        break;
                    }
                }
                self.expect(TokenType::RParen)?;
            }
            _ => {
                // Empty statement is allowed? Or error?
                // Standard PL/0 allows empty statement?
                // Usually not, but let's see.
                // If we return Ok(()), it's an empty statement.
                // But if we see something unexpected, we should error.
                // If current token is End or Semicolon, it might be an empty statement.
                // But `statement` is called when we expect a statement.
                // If we see `End`, it means empty statement before `End`.
                // If we see `;`, it means empty statement before `;`.
                // Let's allow empty statement if it's End or Semicolon?
                // No, `statement` consumes tokens. If it consumes nothing, it's empty.
                // But we need to be careful about infinite loops if we don't consume anything.
                // If we return Ok, the caller continues.
                // If we error, we synchronize.
                // Let's error on unexpected token.
                // But wait, `Begin ... ; End` -> `statement` called on `End`.
                // `End` is not a start of statement.
                // So `statement` should probably return Ok if it sees `End` or `Semicolon` (follow set)?
                // No, `statement` is called *between* semicolons.
                // If `Begin End`, `statement` is called on `End`.
                // So empty statement is valid.
                // But if it's some random token, it's an error.

                // Let's check if it's in the follow set.
                // Follow(statement) = {;, End, Else}
                // If current token is in Follow set, we return Ok (empty statement).
                // Otherwise, error.
                match self.lexer.current_token {
                    TokenType::Semicolon | TokenType::End | TokenType::Else => {}
                    _ => {
                        self.error(&format!(
                            "Unexpected token in statement: {:?}",
                            self.lexer.current_token
                        ))?;
                    }
                }
            }
        }
        Ok(())
    }

    // <lexp> → <exp> <lop> <exp>|odd <exp>
    fn condition(&mut self) -> ParseResult {
        if self.lexer.current_token == TokenType::Odd {
            self.next();
            self.expression()?;
            self.emit(OpCode::OPR, 0, Operator::ODD as i64);
        } else {
            self.expression()?;
            let op = self.lexer.current_token.clone();
            match op {
                TokenType::Equals
                | TokenType::Hash
                | TokenType::LessThan
                | TokenType::LessEqual
                | TokenType::GreaterThan
                | TokenType::GreaterEqual => {
                    self.next();
                    self.expression()?;
                    match op {
                        TokenType::Equals => self.emit(OpCode::OPR, 0, Operator::EQL as i64),
                        TokenType::Hash => self.emit(OpCode::OPR, 0, Operator::NEQ as i64),
                        TokenType::LessThan => self.emit(OpCode::OPR, 0, Operator::LSS as i64),
                        TokenType::GreaterEqual => self.emit(OpCode::OPR, 0, Operator::GEQ as i64),
                        TokenType::GreaterThan => self.emit(OpCode::OPR, 0, Operator::GTR as i64),
                        TokenType::LessEqual => self.emit(OpCode::OPR, 0, Operator::LEQ as i64),
                        _ => {}
                    }
                }
                _ => self.error("Expected relational operator")?,
            }
        }
        Ok(())
    }

    // <exp> → [+|-]<term>{<aop><term>}
    fn expression(&mut self) -> ParseResult {
        let mut op = TokenType::Unknown;
        if self.lexer.current_token == TokenType::Plus
            || self.lexer.current_token == TokenType::Minus
        {
            op = self.lexer.current_token.clone();
            self.next();
        }

        self.term()?;

        if op == TokenType::Minus {
            self.emit(OpCode::OPR, 0, Operator::NEG as i64); // Negate
        }

        while self.lexer.current_token == TokenType::Plus
            || self.lexer.current_token == TokenType::Minus
        {
            op = self.lexer.current_token.clone();
            self.next();
            self.term()?;
            if op == TokenType::Plus {
                self.emit(OpCode::OPR, 0, Operator::ADD as i64);
            } else {
                self.emit(OpCode::OPR, 0, Operator::SUB as i64);
            }
        }
        Ok(())
    }

    // <term> → <factor>{<mop><factor>}
    fn term(&mut self) -> ParseResult {
        self.factor()?;
        while self.lexer.current_token == TokenType::Multiply
            || self.lexer.current_token == TokenType::Divide
        {
            let op = self.lexer.current_token.clone();
            self.next();
            self.factor()?;
            if op == TokenType::Multiply {
                self.emit(OpCode::OPR, 0, Operator::MUL as i64);
            } else {
                self.emit(OpCode::OPR, 0, Operator::DIV as i64);
            }
        }
        Ok(())
    }

    // <factor>→<id>|<integer>|(<exp>)
    fn factor(&mut self) -> ParseResult {
        match self.lexer.current_token.clone() {
            TokenType::Identifier(name) => {
                let line = self.lexer.token_line;
                let col = self.lexer.token_col;
                self.next();
                let (level, addr) = if let Some((_, sym)) = self.position(&name) {
                    match sym.kind {
                        SymbolType::Constant { val } => {
                            self.emit(OpCode::LIT, 0, val);
                            return Ok(());
                        }
                        SymbolType::Variable { level, addr } => (level, addr),
                        _ => {
                            self.error_at(line, col, "Expected constant or variable")?;
                            (0, 0)
                        }
                    }
                } else {
                    self.error_at(line, col, &format!("Undefined symbol: {}", name))?;
                    (0, 0)
                };
                self.emit(OpCode::LOD, self.level - level, addr);
            }
            TokenType::Number(val) => {
                self.next();
                self.emit(OpCode::LIT, 0, val);
            }
            TokenType::LParen => {
                self.next();
                self.expression()?;
                self.expect(TokenType::RParen)?;
            }
            _ => self.error("Expected factor")?,
        }
        Ok(())
    }
}
