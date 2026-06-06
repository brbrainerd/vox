pub use vox_eval::*;

pub mod builtins;
pub mod db;
pub mod env;
pub mod expr;
pub mod shell_stdlib;
pub mod stmt;
pub mod value;

use crate::hir::nodes::HirModule;
use env::Scope;
use value::VoxValue;

#[derive(Debug)]
pub enum EvalError {
    UndefinedVariable(String),
    TypeError {
        expected: &'static str,
        found: String,
    },
    ArityMismatch {
        expected: usize,
        found: usize,
    },
    StepLimitExceeded,
    AssertionFailed(String),
    Panic(String),
}

pub struct Interpreter {
    pub scope: Scope,
    pub module_scope: Scope,
    pub step_limit: usize,
    pub steps: usize,
    pub caps: Option<std::collections::HashSet<String>>,
    /// Absolute path of the file currently being run, used as the resolution
    /// base for intra-project `import "./helpers/foo.vox"` directives.
    /// When `None`, local-file imports are reported as an error.
    pub source_path: Option<std::path::PathBuf>,
    /// Set of canonicalized paths already loaded — guards against import
    /// cycles. A re-entrant resolve sees the path here and aborts with
    /// `EvalError::AssertionFailed` naming the cycle.
    pub loaded_imports: std::collections::HashSet<std::path::PathBuf>,
    /// In-memory database for `db.*` operations under `--mode interp`. Lowered
    /// query plans execute against this store so data-layer programs produce
    /// real input→output in the default run mode. See
    /// [`crate::eval::db`].
    pub db: crate::eval::db::DbStore,
}

impl Interpreter {
    pub fn new(step_limit: usize) -> Self {
        let mut scope = Scope::new();
        // Seed namespaces for glue-code builtins
        scope.set("null".to_string(), VoxValue::Null);
        scope.set(
            "fs".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("fs".to_string()),
            )]),
        );
        scope.set(
            "process".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("process".to_string()),
            )]),
        );
        scope.set(
            "env".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("env".to_string()),
            )]),
        );
        scope.set(
            "path".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("path".to_string()),
            )]),
        );
        scope.set(
            "secrets".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("secrets".to_string()),
            )]),
        );
        scope.set(
            "json".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("json".to_string()),
            )]),
        );
        scope.set(
            "regex".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("regex".to_string()),
            )]),
        );
        scope.set(
            "log".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("log".to_string()),
            )]),
        );
        scope.set(
            "time".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("time".to_string()),
            )]),
        );
        scope.set(
            "io".to_string(),
            VoxValue::object(vec![(
                "__namespace__".to_string(),
                VoxValue::Str("io".to_string()),
            )]),
        );

        // Standard library root
        let std_ns = VoxValue::object(vec![
            (
                "fs".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("fs".to_string()),
                )]),
            ),
            (
                "process".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("process".to_string()),
                )]),
            ),
            (
                "env".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("env".to_string()),
                )]),
            ),
            (
                "path".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("path".to_string()),
                )]),
            ),
            (
                "json".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("json".to_string()),
                )]),
            ),
            (
                "agentos".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("agentos".to_string()),
                )]),
            ),
            (
                "csv".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("csv".to_string()),
                )]),
            ),
            (
                "toml".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("toml".to_string()),
                )]),
            ),
            (
                "yaml".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("yaml".to_string()),
                )]),
            ),
            (
                "io".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("io".to_string()),
                )]),
            ),
            (
                "log".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("log".to_string()),
                )]),
            ),
            (
                "time".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("time".to_string()),
                )]),
            ),
            (
                "http".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("http".to_string()),
                )]),
            ),
            (
                "regex".to_string(),
                VoxValue::object(vec![(
                    "__namespace__".to_string(),
                    VoxValue::Str("regex".to_string()),
                )]),
            ),
        ]);
        scope.set("std".to_string(), std_ns);

        Self {
            scope: scope.clone(),
            module_scope: scope,
            step_limit,
            steps: 0,
            caps: None,
            source_path: None,
            loaded_imports: std::collections::HashSet::new(),
            db: crate::eval::db::DbStore::default(),
        }
    }

    pub fn run_module(&mut self, module: &HirModule) -> Result<(), EvalError> {
        // Seed built-in Option/Result constructors so scripts can write
        // `return Ok("...")` / `Err(msg)` / `Some(v)` / `None` directly.
        // Per closures-and-stdlib alignment 2026-05-23 (corpus run-mode parity).
        for ctor in ["Ok", "Err", "Error", "Some", "None"] {
            let val = VoxValue::Constructor(ctor.to_string());
            self.scope.set(ctor.to_string(), val.clone());
            self.module_scope.set(ctor.to_string(), val);
        }

        // Resolve intra-project Vox-file imports before binding this module's
        // own decls, so a `pub fn` in the importer can shadow an imported one
        // (defining the function in your own file wins).
        // RFC: docs/src/architecture/intra-project-imports-rfc-2026-05-23.md.
        for imp in &module.imports {
            if let Some(rel_path) = imp.local_file_path.as_ref() {
                self.resolve_local_file_import(rel_path, imp.local_file_alias.as_deref())?;
            }
        }

        // Register ADT variant constructors
        for t in &module.types {
            for variant in &t.variants {
                let val = VoxValue::Constructor(variant.name.clone());
                self.scope.set(variant.name.clone(), val.clone());
                self.module_scope.set(variant.name.clone(), val);
            }
        }

        // Register all functions in both scopes
        for f in &module.functions {
            let val = VoxValue::Fn {
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                body: std::rc::Rc::new(f.body.clone()),
                env: self.scope.clone(),
            };
            self.scope.set(f.name.clone(), val.clone());
            self.module_scope.set(f.name.clone(), val);
        }

        for f in &module.tests {
            let val = VoxValue::Fn {
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                body: std::rc::Rc::new(f.body.clone()),
                env: self.scope.clone(),
            };
            self.scope.set(f.name.clone(), val.clone());
            self.module_scope.set(f.name.clone(), val);
        }

        // Register endpoint handlers (`@query`/`@mutation`/`@endpoint`/`@server`)
        // as ordinary callables. Under `--mode script` these become HTTP
        // handlers, but in the interpreter they are still plain functions —
        // without this a `@query fn` called from `main`/a `@test` failed with
        // `UndefinedVariable`, since endpoint fns live in `module.endpoint_fns`,
        // not `module.functions`.
        for f in &module.endpoint_fns {
            let val = VoxValue::Fn {
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                body: std::rc::Rc::new(f.body.clone()),
                env: self.scope.clone(),
            };
            self.scope.set(f.name.clone(), val.clone());
            self.module_scope.set(f.name.clone(), val);
        }

        Ok(())
    }

    /// Resolve one intra-project `import "./helpers/foo.vox" [as alias]`.
    /// Reads the file relative to `self.source_path`, parses and lowers it,
    /// then merges its `pub fn`s into the current interpreter's scope (or
    /// namespaces them under `alias` when provided). Cycle-safe via
    /// `self.loaded_imports`.
    fn resolve_local_file_import(
        &mut self,
        rel_path: &str,
        alias: Option<&str>,
    ) -> Result<(), EvalError> {
        let base = self.source_path.clone().ok_or_else(|| {
            EvalError::AssertionFailed(format!(
                "Intra-project import `{rel_path}` requires a known source-file location \
                 (interpreter was constructed without a source_path; supply it via \
                 `Interpreter::set_source_path` before calling run_module). \
                 See RFC §4."
            ))
        })?;
        let base_dir = base.parent().unwrap_or(std::path::Path::new("."));
        let joined = base_dir.join(rel_path);
        let canonical = std::fs::canonicalize(&joined).map_err(|e| {
            EvalError::AssertionFailed(format!(
                "Intra-project import `{rel_path}` could not be resolved relative to `{}`: {e}",
                base.display()
            ))
        })?;

        if !self.loaded_imports.insert(canonical.clone()) {
            // Already loaded — idempotent re-import is OK (diamond pattern).
            // Cycle detection is handled by the recursive descent: if we are
            // *currently* in the middle of loading this file, the per-call
            // visited set below catches it before this insertion.
            return Ok(());
        }

        let source = std::fs::read_to_string(&canonical).map_err(|e| {
            EvalError::AssertionFailed(format!(
                "Intra-project import `{rel_path}` (resolved to `{}`): read failed: {e}",
                canonical.display()
            ))
        })?;

        let tokens = crate::lexer::lex(&source);
        let module = crate::parser::parse_script(tokens).map_err(|errs| {
            EvalError::AssertionFailed(format!(
                "Intra-project import `{rel_path}`: parse failed with {} error(s)",
                errs.len()
            ))
        })?;
        let lowered = crate::hir::lower::lower_module(&module);

        // Recurse into the imported file's own imports first, so deeply-nested
        // dependencies are loaded before we register the top-level pubs.
        let saved_source = self.source_path.replace(canonical.clone());
        for imp in &lowered.imports {
            if let Some(nested) = imp.local_file_path.as_ref() {
                // Re-entrancy on the same path here means a real cycle:
                // we are currently mid-resolving `canonical`, and one of its
                // transitive imports points back at it.
                let nested_joined = canonical
                    .parent()
                    .unwrap_or(std::path::Path::new("."))
                    .join(nested);
                if let Ok(nested_canon) = std::fs::canonicalize(&nested_joined)
                    && nested_canon == canonical
                {
                    return Err(EvalError::AssertionFailed(format!(
                        "Intra-project import cycle: `{}` imports itself (via `{}`)",
                        canonical.display(),
                        nested
                    )));
                }
                self.resolve_local_file_import(nested, imp.local_file_alias.as_deref())?;
            }
        }
        self.source_path = saved_source;

        // Register only `pub fn`s from the imported file.
        let mut alias_bindings: Vec<(String, VoxValue)> = Vec::new();
        for f in &lowered.functions {
            if !f.is_pub {
                continue;
            }
            let val = VoxValue::Fn {
                params: f.params.iter().map(|p| p.name.clone()).collect(),
                body: std::rc::Rc::new(f.body.clone()),
                env: self.scope.clone(),
            };
            match alias {
                None => {
                    self.scope.set(f.name.clone(), val.clone());
                    self.module_scope.set(f.name.clone(), val);
                }
                Some(_) => {
                    alias_bindings.push((f.name.clone(), val));
                }
            }
        }
        // Also bring in any `pub` ADT variant constructors so the importing
        // file can construct values of types defined in the imported module.
        for t in &lowered.types {
            if !t.is_pub {
                continue;
            }
            for variant in &t.variants {
                let val = VoxValue::Constructor(variant.name.clone());
                match alias {
                    None => {
                        self.scope.set(variant.name.clone(), val.clone());
                        self.module_scope.set(variant.name.clone(), val);
                    }
                    Some(_) => {
                        alias_bindings.push((variant.name.clone(), val));
                    }
                }
            }
        }

        if let Some(name) = alias {
            // Build a namespace object exposing `alias.fn_name` access.
            let ns = VoxValue::object(alias_bindings);
            self.scope.set(name.to_string(), ns.clone());
            self.module_scope.set(name.to_string(), ns);
        }

        Ok(())
    }

    /// Set the on-disk path of the file being interpreted; required for
    /// intra-project imports (`import "./relative/path.vox"`).
    pub fn set_source_path(&mut self, path: impl Into<std::path::PathBuf>) {
        self.source_path = Some(path.into());
    }

    pub fn call(&mut self, name: &str, args: Vec<VoxValue>) -> Result<VoxValue, EvalError> {
        let val = self
            .scope
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedVariable(name.to_string()))?;
        if let VoxValue::Fn {
            params,
            body,
            mut env,
        } = val
        {
            if params.len() != args.len() {
                return Err(EvalError::ArityMismatch {
                    expected: params.len(),
                    found: args.len(),
                });
            }
            env.push_frame();
            for (p, arg) in params.iter().zip(args) {
                env.set(p.clone(), arg);
            }

            // Temporary variable to hold the old scope context
            let old_scope = self.scope.clone();
            self.scope = env;

            let mut res = VoxValue::Null;
            for s in body.iter() {
                res = stmt::eval_stmt(self, s)?;
                if let VoxValue::_Return(r) = res {
                    res = *r;
                    break;
                }
            }

            self.scope = old_scope;
            Ok(res)
        } else {
            Err(EvalError::TypeError {
                expected: "function",
                found: "other".into(),
            })
        }
    }

    pub fn track_step(&mut self) -> Result<(), EvalError> {
        self.steps += 1;
        if self.steps >= self.step_limit {
            Err(EvalError::StepLimitExceeded)
        } else {
            Ok(())
        }
    }
}
