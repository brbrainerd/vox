#!/bin/sh
# scripts/broker-bypass-lint.sh
#
# Lints for build-broker BYPASSES: code that names the real cargo binary
# directly instead of resolving `cargo` through PATH. The broker
# (crates/vox-cargo-shim) works by PATH interception -- a script that calls
# plain `cargo` is exactly what the broker catches, NOT a bypass. Do not
# "fix" this script to flag bare `cargo`; that would be backwards.
#
# Detected bypass shapes:
#   1. An explicit path to the rustup proxy: `~/.cargo/bin/cargo`,
#      `$HOME/.cargo/bin/cargo`, `$CARGO_HOME/bin/cargo`, or the `.exe`
#      forms -- including Rust code that builds such a path with
#      `PathBuf`/`.join` (matched by the same literal substring).
#   2. `rustup run <toolchain> cargo ...`, which resolves the toolchain
#      cargo directly.
#
# `$CARGO` (which cargo itself sets for child processes, e.g. for build
# scripts) is deliberately NOT flagged: it names whatever cargo invoked the
# current process -- if that invocation went through the broker, `$CARGO`
# *is* the broker. Using it does not bypass anything; it is the recommended
# way for a build script to find "the cargo that is running me".
#
# Scans (default, no positional args): scripts/, .github/workflows/, and
# crates/**/*.rs. `crates/vox-cargo-shim/` and `crates/vox-build-queue/` are
# inherently exempt -- they *are* the broker and must resolve the real cargo
# by path to do their job. This script itself and its fixtures directory
# (scripts/broker-bypass-fixtures/) are also excluded from the default scan
# since they intentionally contain bypass text for self-testing/docs.
#
# Usage:
#   scripts/broker-bypass-lint.sh                     # scan the committed tree
#   scripts/broker-bypass-lint.sh <path> [<path> ...] # scan only these paths
#   scripts/broker-bypass-lint.sh --allow-file <path> # also allow a whole file
#
# A checked-in allowlist at scripts/broker-bypass-allowlist.txt records
# known, honestly-not-fixed bypasses with a reason so they stay visible
# instead of being silently skipped. One entry per line, colon-delimited:
#   path:line:reason text...   (exempts one line)
#   path:reason text...        (exempts the whole file)
# Blank lines and lines starting with `#` are ignored.

set -eu

SELF_PATH="scripts/broker-bypass-lint.sh"
DEFAULT_ALLOWLIST="scripts/broker-bypass-allowlist.txt"

usage() {
    cat <<'EOF'
Usage: broker-bypass-lint.sh [--allow-file <path>]... [path]...

Scan for build-broker bypasses (direct calls to the real cargo binary
instead of PATH-resolved `cargo`).

With no positional [path] arguments, scans the default committed set:
scripts/, .github/workflows/, crates/**/*.rs (excluding the broker's own
crates, which are inherently exempt).

With one or more [path] arguments, scans only those files/directories
instead -- used for self-test fixtures.

Options:
  --allow-file <path>  Exempt this whole file in addition to the checked-in
                        allowlist (scripts/broker-bypass-allowlist.txt).
                        May be repeated.
  --help                Show this help.

Exit status: 0 if no unallowlisted bypasses found, 1 otherwise.
EOF
}

CDPATH=''
SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
cd -- "$REPO_ROOT"

CLI_ALLOW_TMP=$(mktemp)
SCAN_PATHS_TMP=$(mktemp)
FILES_TMP=$(mktemp)
MATCHES_TMP=$(mktemp)
# shellcheck disable=SC2329 # invoked indirectly via `trap`
cleanup() {
    rm -f "$CLI_ALLOW_TMP" "$SCAN_PATHS_TMP" "$FILES_TMP" "$MATCHES_TMP"
}
trap cleanup EXIT INT TERM

while [ $# -gt 0 ]; do
    case "$1" in
        --allow-file)
            if [ $# -lt 2 ]; then
                echo "broker-bypass-lint.sh: --allow-file requires a path argument" >&2
                exit 2
            fi
            printf '%s\n' "$2" >>"$CLI_ALLOW_TMP"
            shift 2
            ;;
        --help | -h)
            usage
            exit 0
            ;;
        --)
            shift
            while [ $# -gt 0 ]; do
                printf '%s\n' "$1" >>"$SCAN_PATHS_TMP"
                shift
            done
            ;;
        -*)
            echo "broker-bypass-lint.sh: unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
        *)
            printf '%s\n' "$1" >>"$SCAN_PATHS_TMP"
            shift
            ;;
    esac
done

# --- Build the file list to scan --------------------------------------------

if [ -s "$SCAN_PATHS_TMP" ]; then
    while IFS= read -r p; do
        [ -n "$p" ] || continue
        if [ -d "$p" ]; then
            find "$p" -type f >>"$FILES_TMP"
        elif [ -f "$p" ]; then
            printf '%s\n' "$p" >>"$FILES_TMP"
        fi
    done <"$SCAN_PATHS_TMP"
else
    if [ -d scripts ]; then
        find scripts -type f \
            ! -path "$SELF_PATH" \
            ! -path "$DEFAULT_ALLOWLIST" \
            ! -path 'scripts/broker-bypass-fixtures/*' >>"$FILES_TMP"
    fi
    if [ -d .github/workflows ]; then
        find .github/workflows -type f >>"$FILES_TMP"
    fi
    if [ -d crates ]; then
        find crates -type f -name '*.rs' \
            ! -path '*/vox-cargo-shim/*' \
            ! -path '*/vox-build-queue/*' >>"$FILES_TMP"
    fi
fi

# --- Allowlist lookup --------------------------------------------------------
# Checks scripts/broker-bypass-allowlist.txt (path:line:reason or
# path:reason lines) plus any --allow-file paths given on the command line.
# On a hit, prints the recorded reason and returns 0; otherwise returns 1.

is_allowlisted() {
    match_file=$1
    match_line=$2
    if [ -f "$DEFAULT_ALLOWLIST" ]; then
        reason=$(awk -F: -v f="$match_file" -v l="$match_line" '
            /^[[:space:]]*#/ { next }
            /^[[:space:]]*$/ { next }
            {
                path = $1
                if ($2 ~ /^[0-9]+$/) {
                    if (path == f && $2 == l) {
                        out = $3
                        for (i = 4; i <= NF; i++) { out = out ":" $i }
                        print out
                        exit
                    }
                } else {
                    if (path == f) {
                        out = $2
                        for (i = 3; i <= NF; i++) { out = out ":" $i }
                        print out
                        exit
                    }
                }
            }
        ' "$DEFAULT_ALLOWLIST")
        if [ -n "$reason" ]; then
            printf '%s\n' "$reason"
            return 0
        fi
    fi
    if [ -s "$CLI_ALLOW_TMP" ] && grep -qxF "$match_file" "$CLI_ALLOW_TMP"; then
        printf '%s\n' "cli-provided --allow-file"
        return 0
    fi
    return 1
}

# --- Scan --------------------------------------------------------------------

# Case 1: explicit path to the rustup-proxy cargo binary (shell or Rust).
# shellcheck disable=SC2016 # single-quoted on purpose: this is a grep -E pattern, not a shell expansion
PATTERN_PATH='(\.cargo/bin/cargo(\.exe)?|\$CARGO_HOME/bin/cargo(\.exe)?)'
REASON_PATH='names the real cargo binary by path, bypassing the broker (case 1)'

# Case 2: rustup run <toolchain> cargo ..., resolving the toolchain cargo directly.
PATTERN_RUSTUP='rustup[[:space:]]+run[[:space:]]+[^[:space:]]+[[:space:]]+cargo([[:space:]]|$)'
REASON_RUSTUP='rustup run resolves the toolchain cargo directly, bypassing the broker (case 2)'

if [ -s "$FILES_TMP" ]; then
    sort -u "$FILES_TMP" >"$FILES_TMP.sorted"
    mv "$FILES_TMP.sorted" "$FILES_TMP"
    while IFS= read -r f; do
        [ -n "$f" ] || continue
        [ -f "$f" ] || continue
        grep -nE "$PATTERN_PATH" "$f" 2>/dev/null | while IFS=: read -r ln _; do
            printf '%s:%s:%s\n' "$f" "$ln" "$REASON_PATH" >>"$MATCHES_TMP"
        done
        grep -nE "$PATTERN_RUSTUP" "$f" 2>/dev/null | while IFS=: read -r ln _; do
            printf '%s:%s:%s\n' "$f" "$ln" "$REASON_RUSTUP" >>"$MATCHES_TMP"
        done
    done <"$FILES_TMP"
fi

FOUND=0
ALLOWED_COUNT=0

if [ -s "$MATCHES_TMP" ]; then
    sort -u "$MATCHES_TMP" >"$MATCHES_TMP.sorted"
    mv "$MATCHES_TMP.sorted" "$MATCHES_TMP"
    while IFS=: read -r mf ml reason; do
        [ -n "$mf" ] || continue
        if allow_reason=$(is_allowlisted "$mf" "$ml"); then
            echo "$mf:$ml: ALLOWLISTED: $reason ($allow_reason)"
            ALLOWED_COUNT=$((ALLOWED_COUNT + 1))
        else
            echo "$mf:$ml: $reason"
            FOUND=$((FOUND + 1))
        fi
    done <"$MATCHES_TMP"
fi

echo "broker-bypass-lint: $FOUND unallowlisted bypass(es), $ALLOWED_COUNT allowlisted" >&2

if [ "$FOUND" -gt 0 ]; then
    exit 1
fi
exit 0
