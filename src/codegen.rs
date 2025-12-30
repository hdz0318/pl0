use crate::ast::*;
use crate::symbol_table::SymbolTable;
use crate::types::{Instruction, OpCode, Operator, Symbol, SymbolType};

pub struct CodeGenerator {
    code: Vec<Instruction>,
    symbol_table: SymbolTable,
    level: usize,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            symbol_table: SymbolTable::new(),
            level: 0,
        }
    }

    pub fn generate(&mut self, program: &Program) -> Result<Vec<Instruction>, String> {
        self.generate_block(&program.block)?;
        Ok(self.code.clone())
    }

    fn emit(&mut self, f: OpCode, l: usize, a: i64) {
        self.code.push(Instruction::new(f, l, a));
    }

    fn generate_block(&mut self, block: &Block) -> Result<(), String> {
        let jmp_addr = self.code.len();
        self.emit(OpCode::JMP, 0, 0); // Placeholder

        // Declare constants
        for const_decl in &block.consts {
            self.symbol_table.define(Symbol {
                name: const_decl.name.clone(),
                kind: SymbolType::Constant {
                    val: const_decl.value,
                },
            })?;
        }

        // Declare variables
        let mut var_offset = 3; // SL, DL, RA
        for var_name in &block.vars {
            self.symbol_table.define(Symbol {
                name: var_name.clone(),
                kind: SymbolType::Variable {
                    level: self.level,
                    addr: var_offset,
                },
            })?;
            var_offset += 1;
        }

        // Declare procedures
        for proc_decl in &block.procedures {
            let proc_addr = self.code.len();
            self.symbol_table.define(Symbol {
                name: proc_decl.name.clone(),
                kind: SymbolType::Procedure {
                    level: self.level,
                    addr: proc_addr as i64,
                },
            })?;

            self.level += 1;
            self.symbol_table.enter_scope();

            // Define parameters
            let param_count = proc_decl.params.len();
            for (i, param_name) in proc_decl.params.iter().enumerate() {
                // Params are at negative offsets relative to base
                // Last param is at -1, First param is at -param_count
                // i=0 (first) -> offset = -(param_count - 0) = -param_count
                // i=last -> offset = -1
                let offset = -((param_count - i) as i64);
                self.symbol_table.define(Symbol {
                    name: param_name.clone(),
                    kind: SymbolType::Variable {
                        level: self.level,
                        addr: offset,
                    },
                })?;
            }

            self.generate_block(&proc_decl.block)?;
            self.symbol_table.exit_scope();
            self.level -= 1;

            self.emit(OpCode::OPR, 0, Operator::RET as i64);
        }

        // Fix JMP
        self.code[jmp_addr].a = self.code.len() as i64;

        // Allocate space
        self.emit(OpCode::INT, 0, var_offset as i64);

        self.generate_statement(&block.statement)?;

        self.emit(OpCode::OPR, 0, Operator::RET as i64);

        Ok(())
    }

    fn generate_statement(&mut self, stmt: &Statement) -> Result<(), String> {
        match stmt {
            Statement::Assignment { name, expr } => {
                self.generate_expr(expr)?;
                if let Some(sym) = self.symbol_table.resolve(name) {
                    if let SymbolType::Variable { level, addr } = sym.kind {
                        self.emit(OpCode::STO, self.level - level, addr);
                    } else {
                        return Err(format!("Cannot assign to non-variable '{}'", name));
                    }
                } else {
                    return Err(format!("Undefined variable '{}'", name));
                }
            }
            Statement::Call { name, args } => {
                if let Some(sym) = self.symbol_table.resolve(name) {
                    if let SymbolType::Procedure { level, addr } = sym.kind {
                        // Push arguments
                        for arg in args {
                            self.generate_expr(arg)?;
                        }
                        self.emit(OpCode::CAL, self.level - level, addr);
                        // Pop arguments
                        if !args.is_empty() {
                            self.emit(OpCode::INT, 0, -(args.len() as i64));
                        }
                    } else {
                        return Err(format!("'{}' is not a procedure", name));
                    }
                } else {
                    return Err(format!("Undefined procedure '{}'", name));
                }
            }
            Statement::BeginEnd { statements } => {
                for s in statements {
                    self.generate_statement(s)?;
                }
            }
            Statement::If {
                condition,
                then_stmt,
                else_stmt,
            } => {
                self.generate_condition(condition)?;
                let jpc_addr = self.code.len();
                self.emit(OpCode::JPC, 0, 0);
                self.generate_statement(then_stmt)?;

                if let Some(else_s) = else_stmt {
                    let jmp_addr = self.code.len();
                    self.emit(OpCode::JMP, 0, 0);
                    self.code[jpc_addr].a = self.code.len() as i64;
                    self.generate_statement(else_s)?;
                    self.code[jmp_addr].a = self.code.len() as i64;
                } else {
                    self.code[jpc_addr].a = self.code.len() as i64;
                }
            }
            Statement::While { condition, body } => {
                let start_addr = self.code.len();
                self.generate_condition(condition)?;
                let jpc_addr = self.code.len();
                self.emit(OpCode::JPC, 0, 0);
                self.generate_statement(body)?;
                self.emit(OpCode::JMP, 0, start_addr as i64);
                self.code[jpc_addr].a = self.code.len() as i64;
            }
            Statement::Read { names } => {
                for name in names {
                    if let Some(sym) = self.symbol_table.resolve(name) {
                        if let SymbolType::Variable { level, addr } = sym.kind {
                            self.emit(OpCode::RED, self.level - level, addr);
                        } else {
                            return Err(format!("Cannot read into non-variable '{}'", name));
                        }
                    } else {
                        return Err(format!("Undefined variable '{}'", name));
                    }
                }
            }
            Statement::Write { exprs } => {
                for expr in exprs {
                    self.generate_expr(expr)?;
                    self.emit(OpCode::WRT, 0, 0);
                }
            }
            Statement::Empty => {}
        }
        Ok(())
    }

    fn generate_condition(&mut self, cond: &Condition) -> Result<(), String> {
        match cond {
            Condition::Odd { expr } => {
                self.generate_expr(expr)?;
                self.emit(OpCode::OPR, 0, Operator::ODD as i64);
            }
            Condition::Compare { left, op, right } => {
                self.generate_expr(left)?;
                self.generate_expr(right)?;
                self.emit(OpCode::OPR, 0, *op as i64);
            }
        }
        Ok(())
    }

    fn generate_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Binary { left, op, right } => {
                self.generate_expr(left)?;
                self.generate_expr(right)?;
                self.emit(OpCode::OPR, 0, *op as i64);
            }
            Expr::Unary { op, expr } => {
                self.generate_expr(expr)?;
                self.emit(OpCode::OPR, 0, *op as i64);
            }
            Expr::Number(val) => {
                self.emit(OpCode::LIT, 0, *val);
            }
            Expr::Identifier(name) => {
                if let Some(sym) = self.symbol_table.resolve(name) {
                    match sym.kind {
                        SymbolType::Constant { val } => {
                            self.emit(OpCode::LIT, 0, val);
                        }
                        SymbolType::Variable { level, addr } => {
                            self.emit(OpCode::LOD, self.level - level, addr);
                        }
                        _ => return Err(format!("'{}' is not a value", name)),
                    }
                } else {
                    return Err(format!("Undefined identifier '{}'", name));
                }
            }
        }
        Ok(())
    }
}
