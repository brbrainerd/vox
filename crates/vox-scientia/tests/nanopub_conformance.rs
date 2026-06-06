//! Conformance test: prove `vox_scientia::nanopub::spec::validate_offline` agrees
//! with REAL signed nanopublication vectors from the official upstream test suite.
//!
//! Every fixture under `valid/` is a genuine RSA-signed nanopublication taken
//! verbatim from the `Nanopublication/nanopub-testsuite` repository (see
//! `fixtures/nanopub-testsuite/PROVENANCE.md` for source, commit, and license).
//! `validate_offline` must ACCEPT all of them (re-derive the Trusty hash +
//! RSA-verify the embedded signature).
//!
//! Every fixture under `invalid/` is one of those same valid vectors with a
//! single byte mutated (in the signature, the public key, or the assertion text)
//! so the Trusty hash / signature no longer verifies. `validate_offline` must
//! REJECT all of them.
//!
//! The fixture directories are read at test time, so dropping a new `.trig` into
//! either folder extends coverage automatically. An empty `valid/` directory is a
//! hard failure, so the conformance set can never pass vacuously.

use std::fs;
use std::path::{Path, PathBuf};

use vox_scientia::nanopub::spec::validate_offline;

/// Root of the bundled upstream conformance fixtures.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nanopub-testsuite")
}

/// Collect the `.trig` fixture paths directly inside `dir`, sorted for
/// deterministic ordering. Non-`.trig` files (e.g. `PROVENANCE.md`) are skipped.
fn trig_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir({}) failed: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry should be readable").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "trig"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn valid_vectors_are_accepted() {
    let dir = fixtures_root().join("valid");
    let fixtures = trig_fixtures(&dir);

    // Guard against a vacuous pass: an empty fixture set must FAIL.
    assert!(
        !fixtures.is_empty(),
        "no valid nanopub fixtures found under {} — the conformance set must not be empty",
        dir.display()
    );

    for path in &fixtures {
        let trig = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("reading {} failed: {e}", path.display()));
        assert!(
            validate_offline(&trig).is_ok(),
            "validate_offline should ACCEPT genuine signed nanopub {}",
            path.display()
        );
    }
}

#[test]
fn invalid_vectors_are_rejected() {
    let dir = fixtures_root().join("invalid");
    let fixtures = trig_fixtures(&dir);

    assert!(
        !fixtures.is_empty(),
        "no invalid nanopub fixtures found under {} — the rejection set must not be empty",
        dir.display()
    );

    for path in &fixtures {
        let trig = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("reading {} failed: {e}", path.display()));
        assert!(
            validate_offline(&trig).is_err(),
            "validate_offline should REJECT tampered nanopub {}",
            path.display()
        );
    }
}
