use std::collections::HashMap;
use crate::ty::Ty;

/// A single scope level in the type environment.
#[derive(Debug, Clone)]
struct Scope {
    /// Variable/function bindings: name → type
    bindings: HashMap<String, Binding>,
    /// Local type bindings (e.g. generic parameters): name → type
    type_bindings: HashMap<String, Ty>,
}

impl Scope {
    fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            type_bindings: HashMap::new(),
        }
    }
}

/// A named binding in the environment.
#[derive(Debug, Clone)]
pub struct Binding {
    pub ty: Ty,
    pub mutable: bool,
    pub kind: BindingKind,
}

/// What kind of name this binding refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingKind {
    /// A let-bound variable
    Variable,
    /// A function or lambda parameter
    Parameter,
    /// A top-level function
    Function,
    /// An actor name (used with `spawn`)
    Actor,
    /// A type constructor variant (e.g., `Ok`, `Error`, `Connected`)
    Constructor,
    /// An import (external module binding)
    Import,
    /// An activity definition (durable execution)
    Activity,
    /// A table definition (persistent record type)
    Table,
}

/// Registered ADT (Algebraic Data Type) with its variants.
#[derive(Debug, Clone)]
pub struct AdtDef {
    pub name: String,
    /// Each variant: (variant_name, [(field_name, field_type)])
    pub variants: Vec<VariantDef>,
}

/// A single variant of an ADT.
#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    pub fields: Vec<(String, Ty)>,
}

/// Type environment for semantic analysis.
///
/// Tracks scoped variable bindings, registered types (ADTs), and
/// actor/workflow declarations. Supports lexical scoping with
/// push/pop for nested blocks.
#[derive(Debug)]
pub struct TypeEnv {
    /// Stack of lexical scopes (innermost is last)
    scopes: Vec<Scope>,
    /// Registered ADT definitions: type_name → AdtDef
    types: HashMap<String, AdtDef>,
    /// Actor names → handler signatures: actor_name → [(handler_name, params, return_type)]
    actors: HashMap<String, Vec<ActorHandlerSig>>,
    /// Workflow names → (params, return_type)
    workflows: HashMap<String, WorkflowSig>,
    /// Stack of expected return types for currently checked functions
    return_types: Vec<Ty>,
}

/// Signature of an actor handler.
#[derive(Debug, Clone)]
pub struct ActorHandlerSig {
    pub event_name: String,
    pub params: Vec<(String, Ty)>,
    pub return_type: Ty,
}

/// Signature of a workflow.
#[derive(Debug, Clone)]
pub struct WorkflowSig {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    pub return_type: Ty,
}

impl TypeEnv {
    /// Create a new type environment with a single global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            types: HashMap::new(),
            actors: HashMap::new(),
            workflows: HashMap::new(),
            return_types: Vec::new(),
        }
    }

    /// Define a local type alias or generic parameter in the current scope.
    pub fn define_type(&mut self, name: String, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.type_bindings.insert(name, ty);
        }
    }

    /// Look up a type binding in scope stack.
    pub fn lookup_type(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.type_bindings.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    // ── Scope management ──────────────────────────────────────

    /// Push a new lexical scope (e.g., entering a function body or block).
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Pop the innermost lexical scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Enter a function body with a specific return type.
    pub fn push_return_type(&mut self, ty: Ty) {
        self.return_types.push(ty);
    }

    /// Exit a function body.
    pub fn pop_return_type(&mut self) {
        self.return_types.pop();
    }

    /// Get the current expected return type.
    pub fn current_return_type(&self) -> Option<&Ty> {
        self.return_types.last()
    }

    // ── Variable bindings ─────────────────────────────────────

    /// Define a name in the current (innermost) scope.
    pub fn define(&mut self, name: String, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name, binding);
        }
    }

    /// Look up a name, searching from innermost to outermost scope.
    /// Returns `None` if the name is not found in any scope.
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(binding) = scope.bindings.get(name) {
                return Some(binding);
            }
        }
        None
    }

    /// Check if a name is defined in the current (innermost) scope only.
    /// Used to detect shadowing within the same scope.
    pub fn defined_in_current_scope(&self, name: &str) -> bool {
        self.scopes
            .last()
            .map_or(false, |scope| scope.bindings.contains_key(name))
    }

    // ── Type (ADT) registration ───────────────────────────────

    /// Register an ADT definition.
    pub fn register_type(&mut self, def: AdtDef) {
        // Also register each variant as a constructor binding in the global scope
        for variant in &def.variants {
            let ty = if variant.fields.is_empty() {
                // Nullary constructor: it IS the type (e.g., `Disconnected` → `NetworkState`)
                Ty::Named(def.name.clone())
            } else {
                // Constructor with fields: it's a function from fields → type
                let param_types: Vec<Ty> = variant.fields.iter().map(|(_, t)| t.clone()).collect();
                Ty::Fn(param_types, Box::new(Ty::Named(def.name.clone())))
            };
            self.scopes[0].bindings.insert(
                variant.name.clone(),
                Binding {
                    ty,
                    mutable: false,
                    kind: BindingKind::Constructor,
                },
            );
        }
        self.types.insert(def.name.clone(), def);
    }

    /// Look up an ADT definition by name.
    pub fn lookup_adt(&self, name: &str) -> Option<&AdtDef> {
        self.types.get(name)
    }

    /// Get all variant names for a given ADT.
    pub fn variant_names(&self, type_name: &str) -> Vec<String> {
        self.types
            .get(type_name)
            .map(|adt| adt.variants.iter().map(|v| v.name.clone()).collect())
            .unwrap_or_default()
    }

    // ── Actor registration ────────────────────────────────────

    /// Register an actor and its handler signatures.
    pub fn register_actor(&mut self, name: String, handlers: Vec<ActorHandlerSig>) {
        // The actor name itself is a binding (used with `spawn(ActorName)`)
        self.scopes[0].bindings.insert(
            name.clone(),
            Binding {
                ty: Ty::Named(name.clone()),
                mutable: false,
                kind: BindingKind::Actor,
            },
        );
        self.actors.insert(name, handlers);
    }

    /// Look up an actor's handler signatures.
    pub fn lookup_actor(&self, name: &str) -> Option<&Vec<ActorHandlerSig>> {
        self.actors.get(name)
    }

    // ── Workflow registration ─────────────────────────────────

    /// Register a workflow signature.
    pub fn register_workflow(&mut self, sig: WorkflowSig) {
        let name = sig.name.clone();
        let ty = Ty::Fn(
            sig.params.iter().map(|(_, t)| t.clone()).collect(),
            Box::new(sig.return_type.clone()),
        );
        self.scopes[0].bindings.insert(
            name.clone(),
            Binding {
                ty,
                mutable: false,
                kind: BindingKind::Function,
            },
        );
        self.workflows.insert(name, sig);
    }

    /// Look up a workflow signature.
    pub fn lookup_workflow(&self, name: &str) -> Option<&WorkflowSig> {
        self.workflows.get(name)
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_push_pop() {
        let mut env = TypeEnv::new();
        env.define("x".into(), Binding {
            ty: Ty::Int,
            mutable: false,
            kind: BindingKind::Variable,
        });
        assert!(env.lookup("x").is_some());

        env.push_scope();
        // x is still visible from outer scope
        assert!(env.lookup("x").is_some());

        // Shadow x in inner scope
        env.define("x".into(), Binding {
            ty: Ty::Str,
            mutable: false,
            kind: BindingKind::Variable,
        });
        assert_eq!(env.lookup("x").unwrap().ty, Ty::Str);

        env.pop_scope();
        // x is back to Int
        assert_eq!(env.lookup("x").unwrap().ty, Ty::Int);
    }

    #[test]
    fn test_adt_registration() {
        let mut env = TypeEnv::new();
        env.register_type(AdtDef {
            name: "Shape".into(),
            variants: vec![
                VariantDef { name: "Circle".into(), fields: vec![("r".into(), Ty::Float)] },
                VariantDef { name: "Point".into(), fields: vec![] },
            ],
        });

        // ADT is registered
        assert!(env.lookup_adt("Shape").is_some());

        // Constructors are registered as bindings
        let circle = env.lookup("Circle").unwrap();
        assert_eq!(circle.kind, BindingKind::Constructor);
        // Circle(r: float) → Shape — it's a function
        assert!(matches!(circle.ty, Ty::Fn(_, _)));

        let point = env.lookup("Point").unwrap();
        assert_eq!(point.kind, BindingKind::Constructor);
        // Point is nullary — it IS Shape directly
        assert_eq!(point.ty, Ty::Named("Shape".into()));
    }

    #[test]
    fn test_variant_names() {
        let mut env = TypeEnv::new();
        env.register_type(AdtDef {
            name: "Result".into(),
            variants: vec![
                VariantDef { name: "Ok".into(), fields: vec![("value".into(), Ty::Str)] },
                VariantDef { name: "Error".into(), fields: vec![("message".into(), Ty::Str)] },
            ],
        });

        let names = env.variant_names("Result");
        assert_eq!(names, vec!["Ok".to_string(), "Error".to_string()]);
        assert!(env.variant_names("NonExistent").is_empty());
    }

    #[test]
    fn test_actor_registration() {
        let mut env = TypeEnv::new();
        env.register_actor("Worker".into(), vec![
            ActorHandlerSig {
                event_name: "receive".into(),
                params: vec![("msg".into(), Ty::Str)],
                return_type: Ty::Unit,
            },
        ]);

        assert!(env.lookup("Worker").is_some());
        assert_eq!(env.lookup("Worker").unwrap().kind, BindingKind::Actor);
        assert!(env.lookup_actor("Worker").is_some());
    }

    #[test]
    fn test_defined_in_current_scope() {
        let mut env = TypeEnv::new();
        env.define("x".into(), Binding {
            ty: Ty::Int,
            mutable: false,
            kind: BindingKind::Variable,
        });
        assert!(env.defined_in_current_scope("x"));

        env.push_scope();
        // x is visible but NOT defined in current scope
        assert!(!env.defined_in_current_scope("x"));
        assert!(env.lookup("x").is_some());
    }
}
