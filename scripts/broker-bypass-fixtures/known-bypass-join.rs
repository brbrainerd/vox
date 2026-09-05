// Fixture for scripts/broker-bypass-lint.vox's self-test.
//
// This file is NOT excluded from the lint's default scan by accident -- it
// is excluded on purpose (see broker-bypass-lint.vox's fixtures-dir skip)
// because it deliberately contains a real bypass shape, so it must only be
// scanned when passed explicitly:
//
//   vox run scripts/broker-bypass-lint.vox -- scripts/broker-bypass-fixtures/known-bypass-join.rs
//
// and must make the lint exit non-zero with a "path:line:" report line.
//
// Case 3 bypass: a chained-`.join()` refactor of `cargo_bin()` in
// crates/vox-cli-ci/src/helpers.rs -- same runtime behavior as building
// the resolved path with one literal string, but split across separate
// path-segment calls so no single quoted literal spells out the whole path.
use std::path::PathBuf;

pub fn cargo_bin_joined(home: &str) -> PathBuf {
    PathBuf::from(home)
        .join(".cargo")
        .join("bin")
        .join("cargo.exe")
}
