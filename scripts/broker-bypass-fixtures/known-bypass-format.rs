// Fixture for scripts/broker-bypass-lint.vox's self-test.
//
// This file is NOT excluded from the lint's default scan by accident -- it
// is excluded on purpose (see broker-bypass-lint.vox's fixtures-dir skip)
// because it deliberately contains a real bypass shape, so it must only be
// scanned when passed explicitly:
//
//   vox run scripts/broker-bypass-lint.vox -- scripts/broker-bypass-fixtures/known-bypass-format.rs
//
// and must make the lint exit non-zero with a "path:line:" report line.
//
// Case 4 bypass: builds the resolved cargo-bin path with `format!` instead
// of a literal or `.join()` chain -- same runtime behavior, different text
// shape.
use std::env;

pub fn cargo_bin_formatted() -> String {
    format!("{}/bin/{}", env::var("CARGO_HOME").unwrap(), "cargo")
}
