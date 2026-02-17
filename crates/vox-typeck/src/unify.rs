use crate::ty::Ty;

/// Inference context with union-find based type variable substitution.
pub struct InferenceContext {
    substitutions: Vec<Option<Ty>>,
    next_var: u32,
}

impl InferenceContext {
    pub fn new() -> Self {
        Self {
            substitutions: Vec::new(),
            next_var: 0,
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
            Ty::Result(inner) => Ty::Result(Box::new(self.resolve(inner))),
            Ty::Fn(params, ret) => Ty::Fn(
                params.iter().map(|p| self.resolve(p)).collect(),
                Box::new(self.resolve(ret)),
            ),
            Ty::Tuple(elems) => Ty::Tuple(elems.iter().map(|e| self.resolve(e)).collect()),
            _ => ty.clone(),
        }
    }

    /// Unify two types, updating substitutions.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), String> {
        let a = self.resolve(a);
        let b = self.resolve(b);

        match (&a, &b) {
            _ if a == b => Ok(()),
            (Ty::TypeVar(id), _) => {
                self.substitutions[*id as usize] = Some(b);
                Ok(())
            }
            (_, Ty::TypeVar(id)) => {
                self.substitutions[*id as usize] = Some(a);
                Ok(())
            }
            (Ty::List(a_inner), Ty::List(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Option(a_inner), Ty::Option(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Result(a_inner), Ty::Result(b_inner)) => self.unify(a_inner, b_inner),
            (Ty::Fn(a_params, a_ret), Ty::Fn(b_params, b_ret)) => {
                if a_params.len() != b_params.len() {
                    return Err(format!("Function arity mismatch: expected {}, got {}", a_params.len(), b_params.len()));
                }
                for (ap, bp) in a_params.iter().zip(b_params.iter()) {
                    self.unify(ap, bp)?;
                }
                self.unify(a_ret, b_ret)
            }
            (Ty::Tuple(a_elems), Ty::Tuple(b_elems)) => {
                if a_elems.len() != b_elems.len() {
                    return Err(format!("Tuple size mismatch: expected {}, got {}", a_elems.len(), b_elems.len()));
                }
                for (ae, be) in a_elems.iter().zip(b_elems.iter()) {
                    self.unify(ae, be)?;
                }
                Ok(())
            }
            (Ty::Record(a_fields), Ty::Record(b_fields)) => {
                if a_fields.len() != b_fields.len() {
                     return Err(format!("Record size mismatch: expected {}, got {}", a_fields.len(), b_fields.len()));
                }
                // Order-independent structural unification
                for (name, a_ty) in a_fields {
                    if let Some((_, b_ty)) = b_fields.iter().find(|(n, _)| n == name) {
                        self.unify(a_ty, b_ty)?;
                    } else {
                        return Err(format!("Missing field '{}' in record", name));
                    }
                }
                Ok(())
            }
            (Ty::Error, _) | (_, Ty::Error) => Ok(()), // Error recovery
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
        assert!(ctx.unify(&Ty::List(Box::new(var.clone())), &Ty::List(Box::new(Ty::Int))).is_ok());
        assert_eq!(ctx.resolve(&var), Ty::Int);
    }
}
