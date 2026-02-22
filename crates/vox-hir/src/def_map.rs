use crate::hir::DefId;
use std::collections::HashMap;

/// Tracks name → DefId mappings at each scope level.
#[derive(Debug, Clone)]
pub struct DefMap {
    scopes: Vec<Scope>,
    next_id: u32,
}

#[derive(Debug, Clone)]
struct Scope {
    bindings: HashMap<String, DefId>,
}

impl DefMap {
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                bindings: HashMap::new(),
            }],
            next_id: 0,
        }
    }

    pub fn fresh_id(&mut self) -> DefId {
        let id = DefId(self.next_id);
        self.next_id += 1;
        id
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope {
            bindings: HashMap::new(),
        });
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String) -> DefId {
        let id = self.fresh_id();
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, id);
        }
        id
    }

    pub fn lookup(&self, name: &str) -> Option<DefId> {
        for scope in self.scopes.iter().rev() {
            if let Some(id) = scope.bindings.get(name) {
                return Some(*id);
            }
        }
        None
    }
}

impl Default for DefMap {
    fn default() -> Self {
        Self::new()
    }
}
