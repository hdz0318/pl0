use crate::types::Symbol;
use std::collections::HashMap;

pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()], // Start with global scope
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        } else {
            panic!("Cannot exit global scope");
        }
    }

    pub fn define(&mut self, symbol: Symbol) -> Result<(), String> {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&symbol.name) {
                return Err(format!(
                    "Symbol '{}' already defined in current scope",
                    symbol.name
                ));
            }
            scope.insert(symbol.name.clone(), symbol);
            Ok(())
        } else {
            Err("No scope to define symbol".to_string())
        }
    }

    pub fn resolve(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }

    // Helper to get current scope depth (not PL/0 level)
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }
}
