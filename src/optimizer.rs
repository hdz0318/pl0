use crate::ast::*;
use crate::types::Operator;
use std::collections::HashMap;
use std::collections::HashSet;

pub fn optimize_ast(program: &mut Program) {
    optimize_block(&mut program.block);
}

fn optimize_block(block: &mut Block) {
    for proc in &mut block.procedures {
        optimize_block(&mut proc.block);
    }
    optimize_statement(&mut block.statement);
}

fn optimize_statement(stmt: &mut Statement) {
    match stmt {
        Statement::Assignment { expr, .. } => optimize_expr(expr),
        Statement::Call { args, .. } => {
            for arg in args {
                optimize_expr(arg);
            }
        }
        Statement::BeginEnd { statements } => {
            // 1. Optimize children
            for s in statements.iter_mut() {
                optimize_statement(s);
            }

            // 2. DAG / CSE Optimization
            optimize_block_dag(statements);

            // 3. Filter Empty
            let mut new_statements = Vec::new();
            for s in statements.iter() {
                if !matches!(s, Statement::Empty) {
                    new_statements.push(s.clone());
                }
            }
            *statements = new_statements;
        }
        Statement::If {
            condition,
            then_stmt,
            else_stmt,
        } => {
            optimize_condition(condition);
            optimize_statement(then_stmt);
            if let Some(s) = else_stmt {
                optimize_statement(s);
            }

            // Dead Code Elimination for If
            if let Some(val) = evaluate_condition(condition) {
                if val {
                    *stmt = *then_stmt.clone();
                } else if let Some(else_s) = else_stmt {
                    *stmt = *else_s.clone();
                } else {
                    *stmt = Statement::Empty;
                }
            }
        }
        Statement::While { condition, body } => {
            optimize_condition(condition);
            optimize_statement(body);

            // Dead Code Elimination for While
            if let Some(val) = evaluate_condition(condition) {
                if !val {
                    *stmt = Statement::Empty;
                } else {
                    // Loop Invariant Code Motion
                    try_licm(stmt);
                }
            } else {
                // Loop Invariant Code Motion
                try_licm(stmt);
            }
        }
        Statement::Read { .. } => {}
        Statement::Write { exprs } => {
            for expr in exprs {
                optimize_expr(expr);
            }
        }
        Statement::Empty => {}
    }
}

fn evaluate_condition(cond: &Condition) -> Option<bool> {
    match cond {
        Condition::Odd { expr } => {
            if let Expr::Number(val) = expr {
                Some(val % 2 != 0)
            } else {
                None
            }
        }
        Condition::Compare { left, op, right } => {
            if let (Expr::Number(l), Expr::Number(r)) = (left, right) {
                match op {
                    Operator::EQL => Some(l == r),
                    Operator::NEQ => Some(l != r),
                    Operator::LSS => Some(l < r),
                    Operator::LEQ => Some(l <= r),
                    Operator::GTR => Some(l > r),
                    Operator::GEQ => Some(l >= r),
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

fn optimize_condition(cond: &mut Condition) {
    match cond {
        Condition::Odd { expr } => optimize_expr(expr),
        Condition::Compare { left, right, .. } => {
            optimize_expr(left);
            optimize_expr(right);
        }
    }
}

fn optimize_block_dag(statements: &mut Vec<Statement>) {
    let mut available_exprs: HashMap<Expr, String> = HashMap::new();

    for stmt in statements.iter_mut() {
        match stmt {
            Statement::Assignment { name, expr } => {
                // 1. CSE
                let mut replaced = false;
                if let Some(var_name) = available_exprs.get(expr) {
                    *expr = Expr::Identifier(var_name.clone());
                    replaced = true;
                }

                // 2. Invalidate
                available_exprs.retain(|k, _| !expr_uses_var(k, name));

                // 3. Add (if not replaced and complex)
                if !replaced && !matches!(expr, Expr::Number(_) | Expr::Identifier(_)) {
                    if !expr_uses_var(expr, name) {
                        available_exprs.insert(expr.clone(), name.clone());
                    }
                }
            }
            Statement::Read { names } => {
                for name in names {
                    available_exprs.retain(|k, _| !expr_uses_var(k, name));
                }
            }
            Statement::Call { .. } => {
                available_exprs.clear();
            }
            Statement::If { .. } | Statement::While { .. } | Statement::BeginEnd { .. } => {
                available_exprs.clear();
            }
            _ => {}
        }
    }
}

fn expr_uses_var(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Binary { left, right, .. } => expr_uses_var(left, var) || expr_uses_var(right, var),
        Expr::Unary { expr, .. } => expr_uses_var(expr, var),
        Expr::Identifier(name) => name == var,
        _ => false,
    }
}

fn try_licm(stmt: &mut Statement) {
    if let Statement::While { condition: _, body } = stmt {
        // 1. Collect modified vars in loop
        let mut modified = HashSet::new();
        collect_modified_vars(body, &mut modified);

        let mut invariant_stmts = Vec::new();

        match body.as_mut() {
            Statement::BeginEnd { statements } => {
                let mut i = 0;
                while i < statements.len() {
                    let mut hoist = false;
                    if let Statement::Assignment { name: _, expr } = &statements[i] {
                        if !expr_depends_on(expr, &modified) {
                            hoist = true;
                        }
                    }

                    if hoist {
                        invariant_stmts.push(statements.remove(i));
                    } else {
                        i += 1;
                    }
                }
            }
            Statement::Assignment { name: _, expr } => {
                if !expr_depends_on(expr, &modified) {
                    invariant_stmts.push(std::mem::replace(body.as_mut(), Statement::Empty));
                }
            }
            _ => {}
        }

        if !invariant_stmts.is_empty() {
            let loop_stmt = std::mem::replace(stmt, Statement::Empty);
            let mut new_block_stmts = invariant_stmts;
            new_block_stmts.push(loop_stmt);
            *stmt = Statement::BeginEnd {
                statements: new_block_stmts,
            };
        }
    }
}

fn collect_modified_vars(stmt: &Statement, modified: &mut HashSet<String>) {
    match stmt {
        Statement::Assignment { name, .. } => {
            modified.insert(name.clone());
        }
        Statement::Read { names } => {
            for n in names {
                modified.insert(n.clone());
            }
        }
        Statement::BeginEnd { statements } => {
            for s in statements {
                collect_modified_vars(s, modified);
            }
        }
        Statement::If {
            then_stmt,
            else_stmt,
            ..
        } => {
            collect_modified_vars(then_stmt, modified);
            if let Some(s) = else_stmt {
                collect_modified_vars(s, modified);
            }
        }
        Statement::While { body, .. } => {
            collect_modified_vars(body, modified);
        }
        _ => {}
    }
}

fn expr_depends_on(expr: &Expr, vars: &HashSet<String>) -> bool {
    match expr {
        Expr::Binary { left, right, .. } => {
            expr_depends_on(left, vars) || expr_depends_on(right, vars)
        }
        Expr::Unary { expr, .. } => expr_depends_on(expr, vars),
        Expr::Identifier(name) => vars.contains(name),
        _ => false,
    }
}

fn optimize_expr(expr: &mut Expr) {
    match expr {
        Expr::Binary { left, op, right } => {
            optimize_expr(left);
            optimize_expr(right);

            // Constant folding
            if let (Expr::Number(l), Expr::Number(r)) = (left.as_ref(), right.as_ref()) {
                let val = match op {
                    Operator::ADD => l + r,
                    Operator::SUB => l - r,
                    Operator::MUL => l * r,
                    Operator::DIV => {
                        if *r != 0 {
                            l / r
                        } else {
                            return;
                        }
                    } // Avoid div by zero
                    _ => return,
                };
                *expr = Expr::Number(val);
                return;
            }

            // Algebraic Simplification
            // x + 0 = x
            if *op == Operator::ADD {
                if let Expr::Number(0) = right.as_ref() {
                    *expr = *left.clone();
                    return;
                }
                if let Expr::Number(0) = left.as_ref() {
                    *expr = *right.clone();
                    return;
                }
            }
            // x - 0 = x
            if *op == Operator::SUB {
                if let Expr::Number(0) = right.as_ref() {
                    *expr = *left.clone();
                    return;
                }
            }
            // x * 1 = x, x * 0 = 0
            if *op == Operator::MUL {
                if let Expr::Number(1) = right.as_ref() {
                    *expr = *left.clone();
                    return;
                }
                if let Expr::Number(1) = left.as_ref() {
                    *expr = *right.clone();
                    return;
                }
                if let Expr::Number(0) = right.as_ref() {
                    *expr = Expr::Number(0);
                    return;
                }
                if let Expr::Number(0) = left.as_ref() {
                    *expr = Expr::Number(0);
                    return;
                }
            }
            // x / 1 = x
            if *op == Operator::DIV {
                if let Expr::Number(1) = right.as_ref() {
                    *expr = *left.clone();
                    return;
                }
            }
        }
        Expr::Unary { op, expr: inner } => {
            optimize_expr(inner);
            if let Expr::Number(val) = inner.as_ref() {
                if *op == Operator::NEG {
                    *expr = Expr::Number(-val);
                }
            }
        }
        _ => {}
    }
}
