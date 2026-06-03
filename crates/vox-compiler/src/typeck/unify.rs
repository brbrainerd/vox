use crate::typeck::ty::Ty;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum PendingConstraint {
    HasField {
        target: Ty,
        field: String,
        result: Ty,
        span: crate::ast::span::Span,
    },
    HasMethod {
        target: Ty,
        method: String,
        result: Ty,
        args: Vec<Ty>,
        span: crate::ast::span::Span,
    },
}

/// Inference context with union-find based type variable substitution.
pub struct InferenceContext {
    substitutions: Vec<Option<Ty>>,
    next_var: u32,
    pub expected_return_ty: Option<Ty>,
    pub pending_constraints: Vec<PendingConstraint>,
}

impl InferenceContext {
    pub fn new() -> Self {
        Self {
            substitutions: Vec::new(),
            next_var: 0,
            expected_return_ty: None,
            pending_constraints: Vec::new(),
        }
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> Ty {
        let var = Ty::TypeVar(self.next_var);
        self.substitutions.push(None);
        self.next_var += 1;
        var
    }

    /// Resolve a type by following substitution chains.
    pub fn resolve(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::TypeVar(id) => {
                if let Some(Some(resolved)) = self.substitutions.get(*id as usize) {
                    self.resolve(resolved)
                } else {
                    ty.clone()
                }
            }
            Ty::List(inner) => Ty::List(Box::new(self.resolve(inner))),
            Ty::Option(inner) => Ty::Option(Box::new(self.resolve(inner))),
            Ty::Result(ok, err) => {
                Ty::Result(Box::new(self.resolve(ok)), Box::new(self.resolve(err)))
            }
            Ty::Stream(inner) => Ty::Stream(Box::new(self.resolve(inner))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.resolve(k)), Box::new(self.resolve(v))),
            Ty::Set(inner) => Ty::Set(Box::new(self.resolve(inner))),
            Ty::Fn(params, ret) => Ty::Fn(
                params.iter().map(|p| self.resolve(p)).collect(),
                Box::new(self.resolve(ret)),
            ),
            Ty::Tuple(elems) => Ty::Tuple(elems.iter().map(|e| self.resolve(e)).collect()),
            Ty::Record(fields) => Ty::Record(
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.resolve(t)))
                    .collect(),
            ),
            Ty::Table(name, fields) => Ty::Table(
                name.clone(),
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.resolve(t)))
                    .collect(),
            ),
            Ty::Collection(name, fields) => Ty::Collection(
                name.clone(),
                fields
                    .iter()
                    .map(|(n, t)| (n.clone(), self.resolve(t)))
                    .collect(),
            ),
            _ => ty.clone(),
        }
    }

    /// Replace [`Ty::GenericParam`] nodes with fresh [`Ty::TypeVar`]s (one fresh var per param id).
    ///
    /// Matches the AST pipeline’s `instantiate` so builtins like `use_state` and `List::append` unify
    /// correctly in the HIR Checker.
    pub fn instantiate(&mut self, ty: &Ty) -> Ty {
        let mut map = HashMap::new();
        self.instantiate_inner(ty.clone(), &mut map)
    }

    pub fn instantiate_with(&mut self, ty: &Ty, bindings: &[Ty]) -> Ty {
        let mut map = HashMap::new();
        for (i, b) in bindings.iter().enumerate() {
            map.insert(i as u32, b.clone());
        }
        self.instantiate_inner(ty.clone(), &mut map)
    }

    fn instantiate_inner(&mut self, ty: Ty, map: &mut HashMap<u32, Ty>) -> Ty {
        match ty {
            Ty::GenericParam(id) => map.entry(id).or_insert_with(|| self.fresh_var()).clone(),
            Ty::List(inner) => Ty::List(Box::new(self.instantiate_inner(*inner, map))),
            Ty::Option(inner) => Ty::Option(Box::new(self.instantiate_inner(*inner, map))),
            Ty::Result(ok, err) => Ty::Result(
                Box::new(self.instantiate_inner(*ok, map)),
                Box::new(self.instantiate_inner(*err, map)),
            ),
            Ty::Stream(inner) => Ty::Stream(Box::new(self.instantiate_inner(*inner, map))),
            Ty::Set(inner) => Ty::Set(Box::new(self.instantiate_inner(*inner, map))),
            Ty::Map(k, v) => Ty::Map(
                Box::new(self.instantiate_inner(*k, map)),
                Box::new(self.instantiate_inner(*v, map)),
            ),
            Ty::Fn(params, ret) => Ty::Fn(
                params
                    .into_iter()
                    .map(|p| self.instantiate_inner(p, map))
                    .collect(),
                Box::new(self.instantiate_inner(*ret, map)),
            ),
            Ty::Tuple(elems) => Ty::Tuple(
                elems
                    .into_iter()
                    .map(|e| self.instantiate_inner(e, map))
                    .collect(),
            ),
            Ty::Record(fields) => Ty::Record(
                fields
                    .into_iter()
                    .map(|(n, t)| (n, self.instantiate_inner(t, map)))
                    .collect(),
            ),
            Ty::Table(name, fields) => Ty::Table(
                name,
                fields
                    .into_iter()
                    .map(|(n, t)| (n, self.instantiate_inner(t, map)))
                    .collect(),
            ),
            Ty::Collection(name, fields) => Ty::Collection(
                name,
                fields
                    .into_iter()
                    .map(|(n, t)| (n, self.instantiate_inner(t, map)))
                    .collect(),
            ),
            _ => ty,
        }
    }

    fn occurs(&self, id: u32, ty: &Ty) -> bool {
        match self.resolve(ty) {
            Ty::TypeVar(other_id) => id == other_id,
            Ty::List(inner)
            | Ty::Set(inner)
            | Ty::Stream(inner)
            | Ty::Option(inner) => self.occurs(id, &inner),
            Ty::Result(ok, err) => self.occurs(id, &ok) || self.occurs(id, &err),
            Ty::Map(k, v) => self.occurs(id, &k) || self.occurs(id, &v),
            Ty::Tuple(elems) => elems.iter().any(|e| self.occurs(id, e)),
            Ty::Fn(params, ret) => {
                params.iter().any(|p| self.occurs(id, p)) || self.occurs(id, &ret)
            }
            Ty::Record(fields) | Ty::Table(_, fields) | Ty::Collection(_, fields) => {
                fields.iter().any(|(_, t)| self.occurs(id, t))
            }
            _ => false,
        }
    }

    pub fn least_upper_bound(&mut self, a: Ty, b: Ty) -> Result<Ty, String> {
        let a = self.resolve(&a);
        let b = self.resolve(&b);
        if a == b {
            return Ok(a);
        }
        match (&a, &b) {
            (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => Ok(Ty::Float),
            (Ty::Int, Ty::Decimal) | (Ty::Decimal, Ty::Int) => Ok(Ty::Decimal),
            (Ty::Float, Ty::Decimal) | (Ty::Decimal, Ty::Float) => Ok(Ty::Decimal),
            (Ty::List(ai), Ty::List(bi)) => {
                let inner = self.least_upper_bound(ai.as_ref().clone(), bi.as_ref().clone())?;
                Ok(Ty::List(Box::new(inner)))
            }
            (Ty::TypeVar(id), other) | (other, Ty::TypeVar(id)) => {
                self.unify(&Ty::TypeVar(*id), other)?;
                Ok(other.clone())
            }
            _ => {
                self.unify(&a, &b)?;
                Ok(a)
            }
        }
    }

    /// Unify two types, updating substitutions.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), String> {
        let a = self.resolve(a);
        let b = self.resolve(b);

        match (&a, &b) {
            _ if a == b => Ok(()),
            (Ty::TypeVar(id), _) => {
                if self.occurs(*id, &b) {
                    return Err(format!(
                        "Recursive type unification (occurs check failed): TypeVar({id}) occurs in {b:?}"
                    ));
                }
                self.substitutions[*id as usize] = Some(b);
                Ok(())
            }
            (_, Ty::TypeVar(id)) => {
                if self.occurs(*id, &a) {
                    return Err(format!(
                        "Recursive type unification (occurs check failed): TypeVar({id}) occurs in {a:?}"
                    ));
                }
                self.substitutions[*id as usize] = Some(a);
                Ok(())
            }
            (Ty::List(a_inner), Ty::List(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Option(a_inner), Ty::Option(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Result(a_ok, a_err), Ty::Result(b_ok, b_err)) => {
                self.unify(a_ok, b_ok)?;
                self.unify(a_err, b_err)
            }
            (Ty::Stream(a_inner), Ty::Stream(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Set(a_inner), Ty::Set(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Map(ak, av), Ty::Map(bk, bv)) => {
                self.unify(ak, bk)?;
                self.unify(av, bv)
            }
            (Ty::Fn(a_params, a_ret), Ty::Fn(b_params, b_ret)) => {
                if a_params.len() != b_params.len() {
                    return Err(crate::typeck::diagnostics::msg_function_arity_mismatch(
                        a_params.len(),
                        b_params.len(),
                    ));
                }
                for (ap, bp) in a_params.iter().zip(b_params.iter()) {
                    self.unify(ap, bp)?;
                }
                self.unify(a_ret, b_ret)
            }
            (Ty::Tuple(a_elems), Ty::Tuple(b_elems)) => {
                if a_elems.len() != b_elems.len() {
                    return Err(format!(
                        "Tuple size mismatch: expected {}, got {}",
                        a_elems.len(),
                        b_elems.len()
                    ));
                }
                for (ae, be) in a_elems.iter().zip(b_elems.iter()) {
                    self.unify(ae, be)?;
                }
                Ok(())
            }
            (Ty::Record(a_fields), Ty::Record(b_fields)) => {
                for (name, a_ty) in a_fields {
                    if let Some((_, b_ty)) = b_fields.iter().find(|(n, _)| n == name) {
                        self.unify(a_ty, b_ty)?;
                    } else {
                        return Err(format!("Expected field '{name}' missing from record"));
                    }
                }
                Ok(())
            }
            // A record literal `{a: v, ...}` used where a `Map[str, V]` is expected:
            // unify the key type with `Str` (field names are always strings) and each
            // field value with the map's value type.  This is the standard record-as-map
            // coercion; field names become the runtime keys.
            (Ty::Record(fields), Ty::Map(k, v)) | (Ty::Map(k, v), Ty::Record(fields)) => {
                self.unify(k.as_ref(), &Ty::Str)?;
                for (_, field_ty) in fields {
                    self.unify(field_ty, v.as_ref())?;
                }
                Ok(())
            }
            // `char` in Vox is a single-character string.  The lexer produces
            // `Ty::Str` for both `"x"` and `'x'` (single-quoted), so allowing
            // Str ↔ Char unification is the right semantic: a char annotation
            // accepts any single-char string literal.
            (Ty::Str, Ty::Char) | (Ty::Char, Ty::Str) => Ok(()),
            (Ty::Error, Ty::Error) => Ok(()),
            (Ty::Never, _) | (_, Ty::Never) => Ok(()),
            (Ty::Error, other) | (other, Ty::Error) => Err(format!(
                "Cannot unify error type with {}",
                crate::typeck::ty::ty_display(other)
            )),
            (Ty::ImportPlaceholder(a), Ty::ImportPlaceholder(b)) if a == b => Ok(()),
            (Ty::ImportPlaceholder(a), other) | (other, Ty::ImportPlaceholder(a)) => Err(format!(
                "Cannot unify unresolved import '{}' with {}",
                a,
                crate::typeck::ty::ty_display(other)
            )),
            (Ty::Named(a), Ty::Named(b)) if a == b => Ok(()),
            (Ty::ActorRef(a), Ty::ActorRef(b)) if a == b => Ok(()),
            (Ty::Table(an, af), Ty::Table(bn, bf)) if an == bn => {
                if af.len() != bf.len() {
                    return Err("Table field count mismatch".into());
                }
                for ((na, ta), (nb, tb)) in af.iter().zip(bf.iter()) {
                    if na != nb {
                        return Err(format!("Table field name mismatch: {na} vs {nb}"));
                    }
                    self.unify(ta, tb)?;
                }
                Ok(())
            }
            (Ty::Collection(an, af), Ty::Collection(bn, bf)) if an == bn => {
                if af.len() != bf.len() {
                    return Err("Collection field count mismatch".into());
                }
                for ((na, ta), (nb, tb)) in af.iter().zip(bf.iter()) {
                    if na != nb {
                        return Err(format!("Collection field name mismatch: {na} vs {nb}"));
                    }
                    self.unify(ta, tb)?;
                }
                Ok(())
            }
            // A table/collection name used as a value type is structurally
            // compatible with a record carrying the same fields. This is what
            // lets `fn f(rows: List[HealthEventLog])` accept the result of
            // `db.HealthEventLog.all()`, which the builtins type as
            // `List[Record(fields)]` (see `typeck/builtins.rs`). The entity
            // name IS the row type. Matching is by field name (order-
            // independent) with an exact-count requirement so a record can't
            // satisfy a table it only partially overlaps.
            (Ty::Table(_, tf), Ty::Record(rf))
            | (Ty::Record(rf), Ty::Table(_, tf))
            | (Ty::Collection(_, tf), Ty::Record(rf))
            | (Ty::Record(rf), Ty::Collection(_, tf)) => {
                if tf.len() != rf.len() {
                    return Err(format!(
                        "record has {} field(s) but the entity type has {}",
                        rf.len(),
                        tf.len()
                    ));
                }
                for (name, t_ty) in tf {
                    match rf.iter().find(|(rn, _)| rn == name) {
                        Some((_, r_ty)) => self.unify(t_ty, r_ty)?,
                        None => return Err(format!("record is missing field '{name}'")),
                    }
                }
                Ok(())
            }
            _ => Err(format!("Cannot unify {:?} with {:?}", a, b)),
        }
    }
}

impl Default for InferenceContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unify_same_types() {
        let mut ctx = InferenceContext::new();
        assert!(ctx.unify(&Ty::Int, &Ty::Int).is_ok());
        assert!(ctx.unify(&Ty::Str, &Ty::Str).is_ok());
    }

    #[test]
    fn test_unify_type_var() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();
        assert!(ctx.unify(&var, &Ty::Int).is_ok());
        assert_eq!(ctx.resolve(&var), Ty::Int);
    }

    #[test]
    fn test_unify_mismatch() {
        let mut ctx = InferenceContext::new();
        assert!(ctx.unify(&Ty::Int, &Ty::Str).is_err());
    }

    #[test]
    fn test_unify_list() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();
        assert!(
            ctx.unify(
                &Ty::List(Box::new(var.clone())),
                &Ty::List(Box::new(Ty::Int))
            )
            .is_ok()
        );
        assert_eq!(ctx.resolve(&var), Ty::Int);
    }

    #[test]
    fn test_unify_never_with_concrete_is_ok() {
        let mut ctx = InferenceContext::new();
        assert!(ctx.unify(&Ty::Never, &Ty::Int).is_ok());
    }

    /// A table type unifies with a record carrying the same fields (the row
    /// shape produced by `db.Table.all()`), so the entity name is usable as
    /// the row type — e.g. `fn f(rows: List[HealthEventLog])`.
    #[test]
    fn test_unify_table_with_matching_record_is_ok() {
        let mut ctx = InferenceContext::new();
        let fields = vec![("id".to_string(), Ty::Int), ("name".to_string(), Ty::Str)];
        let table = Ty::Table("User".to_string(), fields.clone());
        let record = Ty::Record(fields);
        assert!(ctx.unify(&table, &record).is_ok());
        assert!(ctx.unify(&record, &table).is_ok(), "symmetric");
    }

    /// Field-order independence: the record may list fields in any order.
    #[test]
    fn test_unify_table_with_reordered_record_is_ok() {
        let mut ctx = InferenceContext::new();
        let table = Ty::Table(
            "User".to_string(),
            vec![("id".to_string(), Ty::Int), ("name".to_string(), Ty::Str)],
        );
        let record = Ty::Record(vec![
            ("name".to_string(), Ty::Str),
            ("id".to_string(), Ty::Int),
        ]);
        assert!(ctx.unify(&table, &record).is_ok());
    }

    /// A record missing a field, carrying an extra field, or with a
    /// conflicting field type must NOT unify — structural compatibility is
    /// exact, not a subtype relation.
    #[test]
    fn test_unify_table_with_mismatched_record_is_error() {
        let table = Ty::Table(
            "User".to_string(),
            vec![("id".to_string(), Ty::Int), ("name".to_string(), Ty::Str)],
        );
        // Missing 'name'.
        let mut ctx = InferenceContext::new();
        assert!(
            ctx.unify(&table, &Ty::Record(vec![("id".to_string(), Ty::Int)]))
                .is_err()
        );
        // Extra field.
        let mut ctx = InferenceContext::new();
        assert!(
            ctx.unify(
                &table,
                &Ty::Record(vec![
                    ("id".to_string(), Ty::Int),
                    ("name".to_string(), Ty::Str),
                    ("extra".to_string(), Ty::Bool),
                ])
            )
            .is_err()
        );
        // Conflicting field type (id: Str vs Int).
        let mut ctx = InferenceContext::new();
        assert!(
            ctx.unify(
                &table,
                &Ty::Record(vec![
                    ("id".to_string(), Ty::Str),
                    ("name".to_string(), Ty::Str),
                ])
            )
            .is_err()
        );
    }

    /// The same compatibility holds for `Ty::Collection` (live query views).
    #[test]
    fn test_unify_collection_with_matching_record_is_ok() {
        let mut ctx = InferenceContext::new();
        let fields = vec![("id".to_string(), Ty::Int)];
        let coll = Ty::Collection("Users".to_string(), fields.clone());
        assert!(ctx.unify(&coll, &Ty::Record(fields)).is_ok());
    }

    #[test]
    fn test_unify_import_placeholder_with_concrete_is_error() {
        let mut ctx = InferenceContext::new();
        assert!(
            ctx.unify(&Ty::ImportPlaceholder("foo".into()), &Ty::Str)
                .is_err()
        );
    }
}
