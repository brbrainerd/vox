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
            let label = args.first().and_then(|v| match v {
                VoxValue::Str(s) => Some(s.as_str()),
                _ => None,
            });
            Ok(VoxValue::Int(interp.repo.snapshot(label)))
        }
        "changes" => Ok(VoxValue::List(
            interp
                .repo
                .changes()
                .iter()
                .map(|c| VoxValue::Int(c.id))
                .collect(),
        )),
        "undo" => Ok(match interp.repo.undo() {
            Some(id) => VoxValue::Int(id),
            None => VoxValue::Null,
        }),
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
    fn empty_store_has_no_changes_and_undo_is_none() {
        let mut store = RepoStore::default();
        assert!(store.changes().is_empty());
        assert_eq!(store.undo(), None);
    }

    #[test]
    fn ids_are_monotone_with_gaps_after_undo() {
        // Mirrors the vox-vcs CasFallback invariant: undo does not rewind the id
        // counter, so re-snapshotting after undo yields a fresh id.
        let mut store = RepoStore::default();
        assert_eq!(store.snapshot(None), 0);
        assert_eq!(store.snapshot(None), 1);
        assert_eq!(store.undo(), Some(1));
        assert_eq!(store.snapshot(None), 2, "id advances, never reuses 1");
        let ids: Vec<i64> = store.changes().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![0, 2]);
    }

    #[test]
    fn unknown_repo_method_is_an_error() {
        let mut interp = Interpreter::new(10_000);
        let err = execute_repo_op(&mut interp, "rebase", vec![]).unwrap_err();
        match err {
            EvalError::AssertionFailed(msg) => {
                assert!(
                    msg.contains("rebase") && msg.contains("unknown method"),
                    "got: {msg}"
                );
            }
            other => panic!("expected AssertionFailed, got {other:?}"),
        }
    }
}
