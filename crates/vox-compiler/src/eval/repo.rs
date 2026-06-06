//! In-memory `repo.*` (VCS) store for `--mode interp`, mirroring [`crate::eval::db`].

use crate::eval::EvalError;
use crate::eval::Interpreter;
use crate::eval::value::VoxValue;

#[derive(Debug, Clone)]
pub struct RepoChange {
    pub id: i64,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RepoStore {
    changes: Vec<RepoChange>,
    next_id: i64,
}

impl RepoStore {
    pub fn snapshot(&mut self, label: Option<&str>) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.changes.push(RepoChange {
            id,
            label: label.map(str::to_owned),
        });
        id
    }

    pub fn changes(&self) -> &[RepoChange] {
        &self.changes
    }

    pub fn undo(&mut self) -> Option<i64> {
        self.changes.pop().map(|c| c.id)
    }
}

/// Interpreter entry for `repo.<method>(...)`. Mirrors `execute_db_plan`.
pub fn execute_repo_op(
    interp: &mut Interpreter,
    method: &str,
    args: Vec<VoxValue>,
) -> Result<VoxValue, EvalError> {
    match method {
        "snapshot" => {
            if args.len() > 1 {
                return Err(EvalError::ArityMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }
            let label = match args.first() {
                None => None,
                Some(VoxValue::Str(s)) => Some(s.as_str()),
                Some(other) => {
                    return Err(EvalError::TypeError {
                        expected: "str",
                        found: format!("{other:?}"),
                    });
                }
            };
            Ok(VoxValue::Int(interp.repo.snapshot(label)))
        }
        "changes" => {
            if !args.is_empty() {
                return Err(EvalError::ArityMismatch {
                    expected: 0,
                    found: args.len(),
                });
            }
            Ok(VoxValue::list(
                interp
                    .repo
                    .changes()
                    .iter()
                    .map(|c| VoxValue::Int(c.id))
                    .collect(),
            ))
        }
        "undo" => {
            if !args.is_empty() {
                return Err(EvalError::ArityMismatch {
                    expected: 0,
                    found: args.len(),
                });
            }
            Ok(match interp.repo.undo() {
                Some(id) => VoxValue::Int(id),
                None => VoxValue::Null,
            })
        }
        other => Err(EvalError::AssertionFailed(format!(
            "repo.{other}: unknown method"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_then_changes_and_undo() {
        let mut store = RepoStore::default();
        let id0 = store.snapshot(Some("first"));
        let id1 = store.snapshot(Some("second"));
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(store.changes().len(), 2);
        assert_eq!(store.undo(), Some(1));
        assert_eq!(store.changes().len(), 1);
    }

    #[test]
    fn snapshot_non_string_arg_errors() {
        let mut interp = crate::eval::Interpreter::new(10_000);
        let result = execute_repo_op(&mut interp, "snapshot", vec![VoxValue::Int(42)]);
        assert!(
            matches!(result, Err(EvalError::TypeError { .. })),
            "expected TypeError for non-string snapshot arg, got {result:?}"
        );
    }

    #[test]
    fn snapshot_too_many_args_errors() {
        let mut interp = crate::eval::Interpreter::new(10_000);
        let result = execute_repo_op(
            &mut interp,
            "snapshot",
            vec![VoxValue::Str("a".into()), VoxValue::Str("b".into())],
        );
        assert!(
            matches!(result, Err(EvalError::ArityMismatch { .. })),
            "expected ArityMismatch for too many snapshot args, got {result:?}"
        );
    }

    #[test]
    fn changes_with_extra_arg_errors() {
        let mut interp = crate::eval::Interpreter::new(10_000);
        let result = execute_repo_op(&mut interp, "changes", vec![VoxValue::Str("x".into())]);
        assert!(
            matches!(result, Err(EvalError::ArityMismatch { .. })),
            "expected ArityMismatch for extra changes arg, got {result:?}"
        );
    }

    #[test]
    fn undo_with_extra_arg_errors() {
        let mut interp = crate::eval::Interpreter::new(10_000);
        let result = execute_repo_op(&mut interp, "undo", vec![VoxValue::Str("x".into())]);
        assert!(
            matches!(result, Err(EvalError::ArityMismatch { .. })),
            "expected ArityMismatch for extra undo arg, got {result:?}"
        );
    }
}
