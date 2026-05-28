//! Output-hash comparator.
//!
//! For each `(path, expected_hex)` pair declared in the `MainEntity`
//! contract, this module reads the file under the sandbox `cwd`, hashes it
//! with SHA3-256 via [`vox_crypto::compliance_hash`], and compares the
//! hex-encoded digest to the expected value.

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct HashCompareOutcome {
    pub all_match: bool,
    pub mismatches: Vec<HashMismatch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HashMismatch {
    pub path: String,
    pub expected_hex: String,
    pub actual_hex: String,
}

/// Returns an [`HashCompareOutcome`] with `all_match = true` iff every
/// `(paths[i], expected_hex[i])` pair matched. Missing files are recorded as
/// mismatches with `actual_hex = "<missing>"`.
pub fn compare_output_hashes(
    cwd: &Path,
    paths: &[String],
    expected_hex: &[String],
) -> std::io::Result<HashCompareOutcome> {
    assert_eq!(
        paths.len(),
        expected_hex.len(),
        "compare_output_hashes: paths/hashes length mismatch is a contract bug; caller must validate"
    );
    let mut mismatches = Vec::new();
    for (p, want) in paths.iter().zip(expected_hex.iter()) {
        let path = cwd.join(p);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let got = hex::encode(vox_crypto::compliance_hash(&bytes));
                if &got != want {
                    mismatches.push(HashMismatch {
                        path: p.clone(),
                        expected_hex: want.clone(),
                        actual_hex: got,
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                mismatches.push(HashMismatch {
                    path: p.clone(),
                    expected_hex: want.clone(),
                    actual_hex: "<missing>".into(),
                });
            }
            Err(e) => return Err(e),
        }
    }
    Ok(HashCompareOutcome {
        all_match: mismatches.is_empty(),
        mismatches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn matching_hash_passes() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("out.txt"), b"hello").unwrap();
        let want = hex::encode(vox_crypto::compliance_hash(b"hello"));
        let outcome = compare_output_hashes(
            dir.path(),
            &["out.txt".into()],
            &[want],
        )
        .unwrap();
        assert!(outcome.all_match);
        assert!(outcome.mismatches.is_empty());
    }

    #[test]
    fn mismatching_hash_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("out.txt"), b"hello").unwrap();
        let outcome = compare_output_hashes(
            dir.path(),
            &["out.txt".into()],
            &["deadbeef".into()],
        )
        .unwrap();
        assert!(!outcome.all_match);
        assert_eq!(outcome.mismatches.len(), 1);
        assert_eq!(outcome.mismatches[0].path, "out.txt");
        assert_eq!(outcome.mismatches[0].expected_hex, "deadbeef");
    }

    #[test]
    fn missing_file_is_recorded_as_mismatch_not_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = compare_output_hashes(
            dir.path(),
            &["nonexistent.txt".into()],
            &["deadbeef".into()],
        )
        .unwrap();
        assert!(!outcome.all_match);
        assert_eq!(outcome.mismatches.len(), 1);
        assert_eq!(outcome.mismatches[0].actual_hex, "<missing>");
    }

    #[test]
    fn multiple_outputs_partial_match() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"alpha").unwrap();
        fs::write(dir.path().join("b.txt"), b"beta").unwrap();
        let alpha_hex = hex::encode(vox_crypto::compliance_hash(b"alpha"));
        let outcome = compare_output_hashes(
            dir.path(),
            &["a.txt".into(), "b.txt".into()],
            &[alpha_hex, "bogus".into()],
        )
        .unwrap();
        assert!(!outcome.all_match);
        assert_eq!(outcome.mismatches.len(), 1);
        assert_eq!(outcome.mismatches[0].path, "b.txt");
    }
}
