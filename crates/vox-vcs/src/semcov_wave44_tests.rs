//! Adversarial unit tests for vox-vcs — wave 44.
//!
//! These tests target real public types and documented contracts; every test
//! documents the specific plausible bug it is designed to catch.

#[cfg(test)]
mod semcov_wave44_tests {
    use crate::backend::{VcsBackend, VcsBackendKind, VcsError, detect};
    use crate::cas_fallback::CasFallback;
    use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
    use std::path::{Path, PathBuf};

    // ── ChangeId display / parsing ────────────────────────────────────────────

    #[test]
    fn change_id_zero_formats_with_six_digits() {
        // Catches: format string using {:?} or %d that drops the "chg-" prefix
        // or collapses zero to an empty/short string.
        assert_eq!(format!("{}", ChangeId(0)), "chg-000000");
    }

    #[test]
    fn change_id_max_u64_does_not_panic() {
        // Catches: integer overflow / panic in Display when the id exceeds the
        // width that the format string was designed for.
        let s = format!("{}", ChangeId(u64::MAX));
        assert!(
            s.starts_with("chg-"),
            "Display must always start with 'chg-', got: {s}"
        );
    }

    #[test]
    fn change_id_display_is_lexicographically_monotone_for_small_ids() {
        // Catches: zero-padding absent — e.g. "chg-9" sorts after "chg-10"
        // in lex order, which breaks log ordering in UIs.
        let ids: Vec<String> = (0u64..=20).map(|n| format!("{}", ChangeId(n))).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(
            ids, sorted,
            "ChangeId Display should be lex-monotone for ids 0..=20"
        );
    }

    #[test]
    fn change_id_equality_uses_inner_value_not_display() {
        // Catches: PartialEq derived on a newtype but the inner type is wrapped
        // differently in a second constructor path.
        let a = ChangeId(7);
        let b = ChangeId(7);
        let c = ChangeId(8);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── CasFallback — snapshot id monotonicity ────────────────────────────────

    #[tokio::test]
    async fn snapshot_ids_never_reuse_after_undo_then_snapshot() {
        // Catches: implementation that decrements next_id on undo, causing the
        // next snapshot to reuse an id that was previously visible to the caller.
        let mut b = CasFallback::new();
        let id1 = b.snapshot(None, vec![]).await.unwrap();
        b.undo().await.unwrap(); // pops id1
        let id2 = b.snapshot(None, vec![]).await.unwrap();
        assert_ne!(id1, id2, "id must not be reused after undo+snapshot");
        assert!(id2.0 > id1.0, "reissued id must be strictly greater");
    }

    #[tokio::test]
    async fn snapshot_increments_id_by_one_each_time() {
        // Catches: off-by-one in next_id initialisation (starts at 1 vs 0) or
        // double-increment on each snapshot call.
        let mut b = CasFallback::new();
        let first = b.snapshot(None, vec![]).await.unwrap().0;
        let second = b.snapshot(None, vec![]).await.unwrap().0;
        assert_eq!(second, first + 1);
    }

    // ── CasFallback — undo edge cases ─────────────────────────────────────────

    #[tokio::test]
    async fn double_undo_second_returns_nothing_to_undo() {
        // Catches: undo decrement that wraps to MAX or panics on underflow
        // instead of returning NothingToUndo.
        let mut b = CasFallback::new();
        b.snapshot(None, vec![]).await.unwrap();
        b.undo().await.unwrap();
        let err = b.undo().await;
        assert!(
            matches!(err, Err(VcsError::NothingToUndo)),
            "second undo on empty stack must be NothingToUndo, got {err:?}"
        );
    }

    #[tokio::test]
    async fn undo_returns_id_of_removed_change() {
        // Catches: undo returning the wrong id (e.g. the id that will be used
        // next rather than the id of the popped change).
        let mut b = CasFallback::new();
        let snapped = b.snapshot(Some("probe"), vec![]).await.unwrap();
        let undone = b.undo().await.unwrap();
        assert_eq!(
            snapped, undone,
            "undo must return the id of the removed change"
        );
    }

    #[tokio::test]
    async fn undo_removes_exactly_the_last_change_not_first() {
        // Catches: undo that pops from the front (queue semantics) instead of
        // the back (stack semantics).
        let mut b = CasFallback::new();
        b.snapshot(Some("keep"), vec![PathBuf::from("keep.rs")])
            .await
            .unwrap();
        let last_id = b
            .snapshot(Some("remove"), vec![PathBuf::from("remove.rs")])
            .await
            .unwrap();
        b.undo().await.unwrap();
        let remaining = b.changes().await.unwrap();
        assert_eq!(
            remaining.len(),
            1,
            "exactly one change must remain after one undo"
        );
        assert_eq!(remaining[0].label.as_deref(), Some("keep"));
        assert_ne!(remaining[0].id, last_id);
    }

    // ── CasFallback — label / path round-trip ─────────────────────────────────

    #[tokio::test]
    async fn snapshot_none_label_stored_as_none_not_empty_string() {
        // Catches: label: label.map(str::to_owned) being replaced with
        // Some(label.unwrap_or_default()) which turns None into Some("").
        let mut b = CasFallback::new();
        b.snapshot(None, vec![]).await.unwrap();
        let changes = b.changes().await.unwrap();
        assert!(
            changes[0].label.is_none(),
            "None label must round-trip as None"
        );
    }

    #[tokio::test]
    async fn snapshot_with_empty_string_label_preserved() {
        // Catches: stripping empty labels to None (over-normalisation).
        let mut b = CasFallback::new();
        b.snapshot(Some(""), vec![]).await.unwrap();
        let changes = b.changes().await.unwrap();
        assert_eq!(
            changes[0].label.as_deref(),
            Some(""),
            "empty-string label must be preserved, not coerced to None"
        );
    }

    #[tokio::test]
    async fn paths_with_special_chars_survive_round_trip() {
        // Catches: path normalisation that strips Unicode or percent-encodes
        // special characters in filenames on the way in/out.
        let paths = vec![
            PathBuf::from("src/über.rs"),
            PathBuf::from("docs/file with spaces.md"),
            PathBuf::from("a/b/../c.rs"), // un-canonicalised
        ];
        let mut b = CasFallback::new();
        b.snapshot(Some("special"), paths.clone()).await.unwrap();
        let got = b.changes().await.unwrap();
        assert_eq!(
            got[0].changed_paths, paths,
            "paths must round-trip byte-for-byte without normalisation"
        );
    }

    // ── VcsBackend default methods (add_remote, create_branch, push) ──────────

    #[tokio::test]
    async fn cas_fallback_add_remote_returns_unavailable() {
        // Catches: default impl accidentally returning Ok(()) for CAS backends,
        // silently hiding that remotes are unsupported.
        let mut b = CasFallback::new();
        let err = b.add_remote("origin", "https://example.com/repo").await;
        assert!(
            matches!(err, Err(VcsError::Unavailable(_))),
            "CasFallback must not support add_remote, got {err:?}"
        );
    }

    #[tokio::test]
    async fn cas_fallback_create_branch_returns_unavailable() {
        // Catches: default impl that silently succeeds for CAS backends,
        // giving callers false confidence that a branch was created.
        let mut b = CasFallback::new();
        let err = b.create_branch("feature/foo").await;
        assert!(
            matches!(err, Err(VcsError::Unavailable(_))),
            "CasFallback must not support create_branch, got {err:?}"
        );
    }

    #[tokio::test]
    async fn cas_fallback_push_returns_unavailable() {
        // Catches: push default silently succeeding and letting the caller
        // believe data was pushed to a remote that doesn't exist.
        let mut b = CasFallback::new();
        let id = b.snapshot(None, vec![]).await.unwrap();
        let err = b.push("origin", "main", id).await;
        assert!(
            matches!(err, Err(VcsError::Unavailable(_))),
            "CasFallback must not support push, got {err:?}"
        );
    }

    // ── detect() ─────────────────────────────────────────────────────────────

    #[test]
    fn detect_nonexistent_path_returns_cas_not_panic() {
        // Catches: detect() calling .exists() on a sub-path of a non-existent
        // root and panicking instead of gracefully returning Cas.
        let result = detect(Path::new("/absolutely/does/not/exist/12345"));
        assert_eq!(result, VcsBackendKind::Cas);
    }

    // ── Diff default ──────────────────────────────────────────────────────────

    #[test]
    fn diff_default_changed_paths_is_empty_vec_not_nil() {
        // Catches: Default impl accidentally constructing a Diff with a sentinel
        // value (e.g. [PathBuf::new()]) instead of a truly empty Vec.
        let d = Diff::default();
        assert!(d.changed_paths.is_empty());
        assert_eq!(d.changed_paths.len(), 0);
    }

    // ── ResolveStrategy exhaustiveness ────────────────────────────────────────

    #[test]
    fn resolve_strategy_variants_are_distinct() {
        // Catches: accidentally collapsing TakeLeft/TakeRight into one variant
        // via a copy-paste in the enum definition.
        assert_ne!(ResolveStrategy::TakeLeft, ResolveStrategy::TakeRight);
        assert_ne!(ResolveStrategy::TakeLeft, ResolveStrategy::Manual);
        assert_ne!(ResolveStrategy::TakeRight, ResolveStrategy::Manual);
    }

    // ── Conflict type ─────────────────────────────────────────────────────────

    #[test]
    fn conflict_sides_preserves_insertion_order() {
        // Catches: an implementation that stores sides in a HashMap/HashSet,
        // silently reordering them and breaking deterministic merge output.
        let c = Conflict {
            path: PathBuf::from("a.rs"),
            sides: vec!["ours".into(), "theirs".into(), "base".into()],
        };
        assert_eq!(c.sides[0], "ours");
        assert_eq!(c.sides[1], "theirs");
        assert_eq!(c.sides[2], "base");
    }

    // ── VcsError display ──────────────────────────────────────────────────────

    #[test]
    fn vcs_error_nothing_to_undo_message_is_stable() {
        // Catches: error message text changing and breaking downstream string
        // matching in CLI / GUI code that parses error output.
        let msg = format!("{}", VcsError::NothingToUndo);
        assert_eq!(msg, "nothing to undo");
    }

    #[test]
    fn vcs_error_unavailable_includes_inner_message() {
        // Catches: Unavailable variant discarding its payload in Display,
        // producing an uninformative "backend unavailable:" with no detail.
        let msg = format!("{}", VcsError::Unavailable("test reason".into()));
        assert!(
            msg.contains("test reason"),
            "Unavailable error must include the inner message, got: {msg}"
        );
    }
}
