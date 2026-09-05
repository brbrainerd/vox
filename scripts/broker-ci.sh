#!/bin/sh
# scripts/broker-ci.sh
#
# Builds, tests, and clippy-checks crates/vox-cargo-shim exactly as CI would.
#
# vox-cargo-shim is excluded from the root Cargo.toml [workspace] -- its
# `cargo`-named binary can't join ordinary workspace cargo resolution
# without risking the same-directory fork-bomb hazard documented at
# resolve_skips_same_dir_sibling_copy in crates/vox-build-queue/src/resolve.rs
# -- so plain `cargo test --workspace` never builds or tests it. That is
# exactly how a manifest defect making the crate completely unbuildable can
# sit undetected: nothing in normal workspace CI ever touches it. Run this
# script whenever crates/vox-cargo-shim or crates/vox-build-queue changes.
#
# Runs, in order (fails fast, prints which step failed):
#   cargo build   --manifest-path crates/vox-cargo-shim/Cargo.toml --release
#   cargo test    --manifest-path crates/vox-cargo-shim/Cargo.toml --all-targets
#   cargo clippy  --manifest-path crates/vox-cargo-shim/Cargo.toml --all-targets -- -D warnings
# then confirms a real `test result:` line with a non-zero passed count
# appeared (a target with no tests prints "running 0 tests" and exits 0 --
# that must not read as success), and confirms the release binary named
# `cargo` (`cargo.exe` on Windows) actually exists.
#
# Usage: sh scripts/broker-ci.sh
#
# This calls cargo three times. Do not run anything else that invokes cargo
# concurrently with this script.
#
# Respects CARGO_TARGET_DIR if set; does not hardcode a target directory.
# vox-cargo-shim declares its own [workspace] table, so with no
# CARGO_TARGET_DIR set its default target dir is crates/vox-cargo-shim/target
# (not the repo root's target/), which is what this script checks by default.

set -eu

CDPATH=''
SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd -- "$REPO_ROOT"

MANIFEST="crates/vox-cargo-shim/Cargo.toml"

BIN_NAME="cargo"
if [ "${OS:-}" = "Windows_NT" ]; then
    BIN_NAME="cargo.exe"
fi

STEP_LOG=$(mktemp)
# shellcheck disable=SC2329 # invoked indirectly via `trap`
cleanup() {
    rm -f "$STEP_LOG"
}
trap cleanup EXIT INT TERM

fail() {
    echo "broker-ci.sh: FAILED at step: $1" >&2
    echo "--- last step output ---" >&2
    cat "$STEP_LOG" >&2
    exit 1
}

run_step() {
    step_name=$1
    shift
    echo "==> $step_name" >&2
    : >"$STEP_LOG"
    if ! "$@" >"$STEP_LOG" 2>&1; then
        fail "$step_name"
    fi
    cat "$STEP_LOG"
}

run_step "cargo build --manifest-path $MANIFEST --release" \
    cargo build --manifest-path "$MANIFEST" --release

run_step "cargo test --manifest-path $MANIFEST --all-targets" \
    cargo test --manifest-path "$MANIFEST" --all-targets

# Assert on the artifact, not the exit code: a target with no tests prints
# "running 0 tests" / "test result: ok. 0 passed" and exits 0.
if ! grep -qE '^test result:' "$STEP_LOG"; then
    echo "broker-ci.sh: NO TESTS RAN (no 'test result:' line in cargo test output)" >&2
    exit 1
fi
if ! grep -E '^test result:' "$STEP_LOG" | grep -qE '[1-9][0-9]* passed'; then
    echo "broker-ci.sh: NO TESTS RAN (every 'test result:' line reports 0 passed)" >&2
    exit 1
fi

run_step "cargo clippy --manifest-path $MANIFEST --all-targets -- -D warnings" \
    cargo clippy --manifest-path "$MANIFEST" --all-targets -- -D warnings

echo "==> confirm the produced binary" >&2
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    TARGET_DIR="$CARGO_TARGET_DIR"
else
    TARGET_DIR="crates/vox-cargo-shim/target"
fi
BIN_PATH="$TARGET_DIR/release/$BIN_NAME"
if [ ! -f "$BIN_PATH" ]; then
    echo "broker-ci.sh: FAILED at step: artifact check" >&2
    echo "expected release binary not found at: $BIN_PATH" >&2
    exit 1
fi
BASE_NAME=$(basename "$BIN_PATH")
if [ "$BASE_NAME" != "cargo" ] && [ "$BASE_NAME" != "cargo.exe" ]; then
    echo "broker-ci.sh: FAILED at step: artifact check" >&2
    echo "binary at $BIN_PATH is not named cargo/cargo.exe (found: $BASE_NAME)" >&2
    exit 1
fi

echo "broker-ci.sh: OK -- build, test, and clippy all passed; binary present at $BIN_PATH" >&2
exit 0
