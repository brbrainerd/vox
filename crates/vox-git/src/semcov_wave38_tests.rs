//! Adversarial tests for vox-git pure functions.
//! Module: semcov_wave38_tests
//!
//! Focuses on: ObjectId parsing, RefName helpers, GitCommit methods,
//! read_only subcommand allowlist enforcement.

#[cfg(test)]
mod semcov_wave38_tests {
    use crate::object::{GitCommit, ObjectId};
    use crate::read_cmd::{GitReadError, read_only};
    use crate::refs::{RefName, RefStatus};

    // ── ObjectId::parse ───────────────────────────────────────────────────────

    #[test]
    fn object_id_parse_rejects_39_chars() {
        // Catches: off-by-one in the `>= 40` length gate accepting 39-char inputs
        let s = "a".repeat(39);
        assert!(ObjectId::parse(s).is_none());
    }

    #[test]
    fn object_id_parse_accepts_exactly_40_chars() {
        // Catches: `> 40` typo instead of `>= 40` rejecting valid SHA-1
        let s = "a".repeat(40);
        assert!(ObjectId::parse(s).is_some());
    }

    #[test]
    fn object_id_parse_accepts_sha256_64_hex() {
        // Catches: hard-coded 40-char length cap that rejects SHA-256 OIDs
        let s = "b".repeat(64);
        assert!(ObjectId::parse(s).is_some());
    }

    #[test]
    fn object_id_parse_rejects_uppercase_g() {
        // Catches: `is_ascii_hexdigit()` subtlety — 'G' is not hex but adjacent to 'F'
        let mut s = "a".repeat(40);
        s.replace_range(5..6, "G");
        assert!(ObjectId::parse(s).is_none());
    }

    #[test]
    fn object_id_parse_rejects_mixed_non_hex_embedded() {
        // Catches: early-exit validator missing a non-hex char in the middle
        let s = format!("{}z{}", "a".repeat(20), "b".repeat(19));
        assert!(ObjectId::parse(s).is_none());
    }

    #[test]
    fn object_id_parse_allows_uppercase_hex() {
        // Catches: validator rejecting uppercase A-F (valid hex)
        let s = "ABCDEF0123456789ABCDEF0123456789ABCDEF01".to_string();
        assert_eq!(s.len(), 40);
        assert!(ObjectId::parse(s).is_some());
    }

    #[test]
    fn object_id_short_on_exact_40() {
        // Catches: `short()` panicking or returning wrong slice on min-length input
        let id = ObjectId::parse("a".repeat(40)).unwrap();
        assert_eq!(id.short().len(), 7);
        assert_eq!(id.short(), "aaaaaaa");
    }

    #[test]
    fn object_id_display_uses_short_not_full() {
        // Catches: Display accidentally emitting the full 40-char hash
        let id = ObjectId::parse("deadbeef".repeat(5)).unwrap();
        let displayed = format!("{id}");
        assert_eq!(displayed.len(), 7, "Display must use short form (7 chars)");
    }

    // ── RefName helpers ───────────────────────────────────────────────────────

    #[test]
    fn branch_with_slash_roundtrips() {
        // Catches: branch() stripping or escaping slashes in nested branch names
        let r = RefName::branch("feature/JIRA-42/my-fix");
        assert_eq!(r.as_branch_name(), Some("feature/JIRA-42/my-fix"));
    }

    #[test]
    fn as_branch_name_returns_none_for_tag_ref() {
        // Catches: strip_prefix logic matching refs/tags/ when refs/heads/ was requested
        let r = RefName::tag("v2.0");
        assert!(r.as_branch_name().is_none());
    }

    #[test]
    fn as_tag_name_returns_none_for_branch_ref() {
        // Catches: symmetric confusion — as_tag_name accepting refs/heads/ prefixes
        let r = RefName::branch("main");
        assert!(r.as_tag_name().is_none());
    }

    #[test]
    fn remote_tracking_preserves_slash_in_branch() {
        // Catches: remote_tracking() truncating branch at the first slash
        let r = RefName::remote_tracking("upstream", "release/1.x");
        assert_eq!(r.as_str(), "refs/remotes/upstream/release/1.x");
    }

    #[test]
    fn ref_name_display_matches_as_str() {
        // Catches: Display impl using a different path than as_str()
        let r = RefName::branch("main");
        assert_eq!(format!("{r}"), r.as_str());
    }

    #[test]
    fn ref_name_new_arbitrary_string_preserved() {
        // Catches: new() normalizing or rejecting non-standard refs
        let raw = "refs/pull/99/merge";
        let r = RefName::new(raw);
        assert_eq!(r.as_str(), raw);
    }

    // ── GitCommit methods ────────────────────────────────────────────────────

    fn make_commit(message: &str, parent_count: usize) -> GitCommit {
        let fake_oid = || ObjectId("a".repeat(40));
        GitCommit {
            id: fake_oid(),
            parents: (0..parent_count).map(|_| fake_oid()).collect(),
            tree_id: fake_oid(),
            message: message.to_string(),
            author_name: "Test".into(),
            author_email: "t@t.com".into(),
            committer_name: "Test".into(),
            committer_email: "t@t.com".into(),
            timestamp: 0,
        }
    }

    #[test]
    fn commit_summary_empty_message_returns_empty_str() {
        // Catches: unwrap() on lines().next() panicking on empty message
        let c = make_commit("", 0);
        assert_eq!(c.summary(), "");
    }

    #[test]
    fn commit_summary_no_trailing_newline() {
        // Catches: summary() including a trailing \r on Windows-style line endings
        let c = make_commit("Fix bug\r\n\r\nBody\r\n", 0);
        // summary() uses str::lines() which strips \r\n, so it must not carry \r
        assert!(!c.summary().contains('\r'), "summary must strip \\r");
    }

    #[test]
    fn commit_is_merge_false_for_root() {
        // Catches: is_merge() returning true when parents vec is empty (wrong len check)
        let c = make_commit("root", 0);
        assert!(!c.is_merge());
    }

    #[test]
    fn commit_is_merge_false_for_single_parent() {
        // Catches: is_merge() treating any non-empty parents as a merge
        let c = make_commit("normal", 1);
        assert!(!c.is_merge());
    }

    #[test]
    fn commit_is_merge_true_for_two_parents() {
        // Catches: is_merge() using == 2 instead of > 1, failing octopus merges
        let c = make_commit("Merge branch 'a' into 'b'", 2);
        assert!(c.is_merge());
    }

    #[test]
    fn commit_is_merge_true_for_octopus_three_parents() {
        // Catches: is_merge() hard-coding `== 2` and rejecting valid octopus merges
        let c = make_commit("Merge branches 'a', 'b', 'c'", 3);
        assert!(c.is_merge());
    }

    // ── read_only allowlist ───────────────────────────────────────────────────

    #[test]
    fn read_only_rejects_empty_subcommand() {
        // Catches: empty args slice causing args.first() == None → "" → Disallowed("")
        // which should still be an error (not a panic or pass-through)
        let tmp = std::env::temp_dir();
        let err = read_only(&tmp, &[]).unwrap_err();
        assert!(
            matches!(err, GitReadError::Disallowed(ref s) if s.is_empty()),
            "expected Disallowed(\"\"), got {err:?}"
        );
    }

    #[test]
    fn read_only_rejects_fetch_not_on_allowlist() {
        // Catches: allowlist accidentally including "fetch" (network write-equivalent)
        let tmp = std::env::temp_dir();
        let err = read_only(&tmp, &["fetch"]).unwrap_err();
        assert!(
            matches!(err, GitReadError::Disallowed(_)),
            "fetch must be disallowed; got {err:?}"
        );
    }

    #[test]
    fn read_only_rejects_checkout() {
        // Catches: "checkout" sneaking onto the allowlist (mutates HEAD/working tree)
        let tmp = std::env::temp_dir();
        let err = read_only(&tmp, &["checkout"]).unwrap_err();
        assert!(
            matches!(err, GitReadError::Disallowed(_)),
            "checkout must be disallowed"
        );
    }

    #[test]
    fn read_only_rejects_reset() {
        // Catches: "reset" not being on the denylist (can destroy staged changes)
        let tmp = std::env::temp_dir();
        let err = read_only(&tmp, &["reset"]).unwrap_err();
        assert!(
            matches!(err, GitReadError::Disallowed(_)),
            "reset must be disallowed"
        );
    }

    #[test]
    fn read_only_rejects_merge() {
        // Catches: "merge" being confused with "log --merges" and allowed
        let tmp = std::env::temp_dir();
        let err = read_only(&tmp, &["merge"]).unwrap_err();
        assert!(
            matches!(err, GitReadError::Disallowed(_)),
            "merge must be disallowed"
        );
    }
}
