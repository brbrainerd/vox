#!/bin/sh
# Fixture for scripts/broker-bypass-lint.sh's self-test -- a regression
# guard, not a bypass. Plain, PATH-resolved `cargo` is exactly the case the
# broker CATCHES (it works by PATH interception), so it must never be
# flagged. If a future edit to broker-bypass-lint.sh makes this fixture
# report a match, that edit has inverted the lint's purpose and must be
# reverted.
#
#   sh scripts/broker-bypass-lint.sh scripts/broker-bypass-fixtures/no-bypass-bare-cargo.sh
#
# must exit 0 with no match lines (only the summary line on stderr).
set -eu

cargo build --release
cargo test --workspace
rustup run stable rustc --version
