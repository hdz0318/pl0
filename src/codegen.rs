use crate::ast::*;
use crate::symbol_table::SymbolTable;
use crate::types::{Instruction, OpCode, Operator, SymbolType};

pub struct CodeGenerator {
    code: Vec<Instruction>,
    level: usize,
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            level: 0,
        }
    }

    pub fn generate(
        &mut self,
        program: &Program,
        symbol_table: &mut SymbolTable,
    ) -> Vec<Instruction> {
        // Ensure we start at root scope
        symbol_table.current_scope_id = 0;
        self.generate_block(&program.block, symbol_table);
        self.emit(OpCode::OPR, 0, Operator::RET as i64);
        self.code.clone()
    }

    fn emit(&mut self, f: OpCode, l: usize, a: i64) {
        self.code.push(Instruction::new(f, l, a));
    }

    fn generate_block(&mut self, block: &Block, symbol_table: &mut SymbolTable) {
        // Enter the scope associated with this block
        if let Some(scope_id) = block.scope_id {
            symbol_table.enter_scope(scope_id);
        } else {
            panic!("Block has no scope ID assigned");
        }

        let jmp_addr = self.code.len();
        self.emit(OpCode::JMP, 0, 0); // Placeholder

        // We don't need to declare constants or vars in symbol table, they are already there.
        // But we need to calculate var_offset for INT instruction.
        // We can count vars in the block.
        let var_count = block.vars.len();
        let var_offset = 3 + var_count;

        // Declare procedures
        for proc_decl in &block.procedures {
            let proc_addr = self.code.len();

            // Update procedure address in symbol table
            let scope = &mut symbol_table.scopes[symbol_table.current_scope_id];
            if let Some(sym) = scope.symbols.get_mut(&proc_decl.name)
                && let SymbolType::Procedure { ref mut addr, .. } = sym.kind {
                    *addr = proc_addr as i64;
                }

            self.level += 1;
            self.generate_block(&proc_decl.block, symbol_table);
            self.level -= 1;

            self.emit(OpCode::OPR, 0, Operator::RET as i64);
        }

        // Fix JMP
        self.code[jmp_addr].a = self.code.len() as i64;

        // Allocate space
        self.emit(OpCode::INT, 0, var_offset as i64);

        self.generate_statement(&block.statement, symbol_table);

        if block.scope_id != Some(0) {
            symbol_table.exit_scope();
        }
    }

    fn generate_statement(&mut self, stmt: &Statement, symbol_table: &mut SymbolTable) {
        match stmt {
            Statement::Assignment { name, expr, .. } => {
                self.generate_expr(expr, symbol_table);
                let sym = symbol_table.resolve(name).expect("Undefined variable");
                match sym.kind {
                    SymbolType::Variable { level, addr } => {
                        self.emit(OpCode::STO, self.level - level, addr);
                    }
                    _ => panic!("Cannot assign to non-variable"),
                }
            }
            Statement::Call { name, args, .. } => {
                for arg in args {
                    self.generate_expr(arg, symbol_table);
                }

                let sym = symbol_table.resolve(name).expect("Undefined procedure");
                match sym.kind {
                    SymbolType::Procedure { level, addr } => {
                        self.emit(OpCode::CAL, self.level - level, addr);
                        if !args.is_empty() {
                            self.emit(OpCode::INT, 0, -(args.len() as i64));
                        }
                    }
                    _ => panic!("Not a procedure"),
                }
            }
            Statement::BeginEnd { statements } => {
                for s in statements {
                    self.generate_statement(s, symbol_table);
                }
            }
            Statement::If {
                condition,
                then_stmt,
                else_stmt,
                ..
            } => {
                self.generate_condition(condition, symbol_table);
                let jpc_idx = self.code.len();
                self.emit(OpCode::JPC, 0, 0);

                self.generate_statement(then_stmt, symbol_table);

                if let Some(else_s) = else_stmt {
                    let jmp_idx = self.code.len();
                    self.emit(OpCode::JMP, 0, 0);
                    self.code[jpc_idx].a = self.code.len() as i64;
                    self.generate_statement(else_s, symbol_table);
                    self.code[jmp_idx].a = self.code.len() as i64;
                } else {
                    self.code[jpc_idx].a = self.code.len() as i64;
                }
            }
            Statement::While { condition, body, .. } => {
                let start_idx = self.code.len();
                self.generate_condition(condition, symbol_table);
                let jpc_idx = self.code.len();
                self.emit(OpCode::JPC, 0, 0);

                self.generate_statement(body, symbol_table);
                self.emit(OpCode::JMP, 0, start_idx as i64);

                self.code[jpc_idx].a = self.code.len() as i64;
            }
            Statement::Read { names, .. } => {
                for name in names {
                    self.emit(OpCode::OPR, 0, Operator::RED as i64);
                    let sym = symbol_table.resolve(name).expect("Undefined variable");
                    match sym.kind {
                        SymbolType::Variable { level, addr } => {
                            self.emit(OpCode::STO, self.level - level, addr);
                        }
                        _ => panic!("Cannot read into non-variable"),
                    }
                }
            }
            Statement::Write { exprs, .. } => {
                for expr in exprs {
                    self.generate_expr(expr, symbol_table);
                    self.emit(OpCode::OPR, 0, Operator::WRT as i64);
                }
            }
            Statement::Empty => {}
        }
    }

    fn generate_expr(&mut self, expr: &Expr, symbol_table: &mut SymbolTable) {
        match expr {
            Expr::Number(n) => {
                self.emit(OpCode::LIT, 0, *n);
            }
            Expr::Identifier(name) => {
                let sym = symbol_table.resolve(name).expect("Undefined identifier");
                match sym.kind {
                    SymbolType::Constant { val } => {
                        self.emit(OpCode::LIT, 0, val);
                    }
                    SymbolType::Variable { level, addr } => {
                        self.emit(OpCode::LOD, self.level - level, addr);
                    }
                    _ => panic!("Identifier is not a value"),
                }
            }
            Expr::Binary { left, op, right } => {
                self.generate_expr(left, symbol_table);
                self.generate_expr(right, symbol_table);
                self.emit(OpCode::OPR, 0, *op as i64);
            }
            Expr::Unary { op, expr } => {
                self.generate_expr(expr, symbol_table);
                self.emit(OpCode::OPR, 0, *op as i64);
            }
        }
    }

    fn generate_condition(&mut self, cond: &Condition, symbol_table: &mut SymbolTable) {
        match cond {
            Condition::Odd { expr } => {
                self.generate_expr(expr, symbol_table);
                self.emit(OpCode::OPR, 0, Operator::ODD as i64);
            }
            Condition::Compare { left, op, right } => {
                self.generate_expr(left, symbol_table);
                self.generate_expr(right, symbol_table);
                self.emit(OpCode::OPR, 0, *op as i64);
            }
        }
    }
}
