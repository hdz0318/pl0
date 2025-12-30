use crate::ast::*;
use crate::symbol_table::SymbolTable;
use crate::types::{Symbol, SymbolType};

pub struct SemanticAnalyzer<'a> {
    symbol_table: &'a mut SymbolTable,
    errors: Vec<String>,
}

impl<'a> SemanticAnalyzer<'a> {
    pub fn new(symbol_table: &'a mut SymbolTable) -> Self {
        Self {
            symbol_table,
            errors: Vec::new(),
        }
    }

    pub fn analyze(&mut self, program: &mut Program) -> Result<(), Vec<String>> {
        // Root scope is already created (id 0)
        // We associate the main block with scope 0
        program.block.scope_id = Some(0);

        // We don't need to create a new scope for the main block because SymbolTable::new() creates one.
        // But we need to make sure we are in it.
        self.symbol_table.current_scope_id = 0;

        self.analyze_block(&mut program.block, 0)?;

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn analyze_block(&mut self, block: &mut Block, level: usize) -> Result<(), Vec<String>> {
        // Declare constants
        for const_decl in &block.consts {
            if let Err(e) = self.symbol_table.define(Symbol {
                name: const_decl.name.clone(),
                kind: SymbolType::Constant {
                    val: const_decl.value,
                },
            }) {
                self.errors.push(e);
            }
        }

        // Declare variables
        let mut var_offset = 3; // SL, DL, RA
        for var_name in &block.vars {
            if let Err(e) = self.symbol_table.define(Symbol {
                name: var_name.clone(),
                kind: SymbolType::Variable {
                    level,
                    addr: var_offset,
                },
            }) {
                self.errors.push(e);
            }
            var_offset += 1;
        }

        // Declare procedures
        for proc_decl in &mut block.procedures {
            // Define procedure in current scope
            // Address will be resolved during codegen or we can assign a placeholder?
            // Codegen calculates address based on code length. We can't know it here easily without generating code.
            // However, for recursive calls, we need to know it exists.
            // We can store a placeholder addr and update it later, or just store that it is a procedure.
            // The current SymbolType::Procedure has an addr field.
            // Let's set it to -1 or 0 and let Codegen update it?
            // Or better: Codegen will update the symbol table with the real address!
            // But Semantic Analysis needs to check calls.

            if let Err(e) = self.symbol_table.define(Symbol {
                name: proc_decl.name.clone(),
                kind: SymbolType::Procedure {
                    level,
                    addr: 0, // Placeholder, updated in Codegen
                },
            }) {
                self.errors.push(e);
            }
        }

        // Now analyze procedure bodies
        for proc_decl in &mut block.procedures {
            let new_scope_id = self.symbol_table.create_scope();
            proc_decl.block.scope_id = Some(new_scope_id);
            self.symbol_table.enter_scope(new_scope_id);

            // Define parameters
            let param_count = proc_decl.params.len();
            for (i, param_name) in proc_decl.params.iter().enumerate() {
                let offset = -((param_count - i) as i64);
                if let Err(e) = self.symbol_table.define(Symbol {
                    name: param_name.clone(),
                    kind: SymbolType::Variable {
                        level: level + 1,
                        addr: offset,
                    },
                }) {
                    self.errors.push(e);
                }
            }

            self.analyze_block(&mut proc_decl.block, level + 1)?;
            self.symbol_table.exit_scope();
        }

        self.analyze_statement(&block.statement)?;

        Ok(())
    }

    fn analyze_statement(&mut self, stmt: &Statement) -> Result<(), Vec<String>> {
        match stmt {
            Statement::Assignment { name, expr, line } => {
                match self.symbol_table.resolve(name) {
                    Some(sym) => match sym.kind {
                        SymbolType::Constant { .. } => {
                            self.errors.push(format!(
                                "Line {}: Cannot assign to constant '{}'",
                                line, name
                            ));
                        }
                        SymbolType::Procedure { .. } => {
                            self.errors.push(format!(
                                "Line {}: Cannot assign to procedure '{}'",
                                line, name
                            ));
                        }
                        SymbolType::Variable { .. } => {}
                    },
                    None => {
                        self.errors
                            .push(format!("Line {}: Undefined variable '{}'", line, name));
                    }
                }
                self.analyze_expr(expr)?;
            }
            Statement::Call { name, args, line } => {
                match self.symbol_table.resolve(name) {
                    Some(sym) => {
                        match sym.kind {
                            SymbolType::Procedure { .. } => {
                                // Check arg count if we had that info in SymbolType
                            }
                            _ => {
                                self.errors.push(format!(
                                    "Line {}: '{}' is not a procedure",
                                    line, name
                                ));
                            }
                        }
                    }
                    None => {
                        self.errors
                            .push(format!("Line {}: Undefined procedure '{}'", line, name));
                    }
                }
                for arg in args {
                    self.analyze_expr(arg)?;
                }
            }
            Statement::BeginEnd { statements } => {
                for s in statements {
                    self.analyze_statement(s)?;
                }
            }
            Statement::If {
                condition,
                then_stmt,
                else_stmt,
                line: _,
            } => {
                self.analyze_condition(condition)?;
                self.analyze_statement(then_stmt)?;
                if let Some(s) = else_stmt {
                    self.analyze_statement(s)?;
                }
            }
            Statement::While {
                condition,
                body,
                line: _,
            } => {
                self.analyze_condition(condition)?;
                self.analyze_statement(body)?;
            }
            Statement::Read { names, line } => {
                for name in names {
                    match self.symbol_table.resolve(name) {
                        Some(sym) => {
                            if let SymbolType::Constant { .. } = sym.kind {
                                self.errors.push(format!(
                                    "Line {}: Cannot read into constant '{}'",
                                    line, name
                                ));
                            }
                            if let SymbolType::Procedure { .. } = sym.kind {
                                self.errors.push(format!(
                                    "Line {}: Cannot read into procedure '{}'",
                                    line, name
                                ));
                            }
                        }
                        None => {
                            self.errors
                                .push(format!("Line {}: Undefined variable '{}'", line, name));
                        }
                    }
                }
            }
            Statement::Write { exprs, line: _ } => {
                for expr in exprs {
                    self.analyze_expr(expr)?;
                }
            }
            Statement::Empty => {}
        }
        Ok(())
    }

    fn analyze_expr(&mut self, expr: &Expr) -> Result<(), Vec<String>> {
        match expr {
            Expr::Number(_) => {}
            Expr::Identifier(name) => {
                if self.symbol_table.resolve(name).is_none() {
                    self.errors.push(format!("Undefined identifier '{}'", name));
                }
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left)?;
                self.analyze_expr(right)?;
            }
            Expr::Unary { expr, .. } => {
                self.analyze_expr(expr)?;
            }
        }
        Ok(())
    }

    fn analyze_condition(&mut self, cond: &Condition) -> Result<(), Vec<String>> {
        match cond {
            Condition::Odd { expr } => self.analyze_expr(expr),
            Condition::Compare { left, right, .. } => {
                self.analyze_expr(left)?;
                self.analyze_expr(right)
            }
        }
    }
}
