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
#      forms -- including Rust code that builds such a path as one string
#      literal (matched by the same literal substring).
#   2. `rustup run <toolchain> cargo ...` (also across a `\`-continued
#      line), and `rustup which cargo` used to capture the resolved path
#      for later use (e.g. `REAL_CARGO=$(rustup which cargo)`), both of
#      which resolve the toolchain cargo directly.
#   3. Rust code that builds a resolved cargo-bin path via chained
#      `PathBuf`/`.join()` calls, naming the binary itself as the final
#      segment -- `.join("cargo")` or `.join("cargo.exe")` -- even when the
#      chain is split across several `.join()` calls (a real instance, and
#      a plausible rustfmt-driven refactor of it, is
#      `crates/vox-cli-ci/src/helpers.rs`'s `cargo_bin()`, which builds
#      `~/.cargo/bin/cargo.exe` this way on Windows). `.join(".cargo")`
#      alone is deliberately NOT flagged -- see the case-3 pattern comment
#      below for why.
#   4. Rust `format!`/string-template code that assembles a `<dir>/bin/cargo`
#      path piecewise, e.g. `format!("{}/bin/{}", env::var("CARGO_HOME")..,
#      "cargo")` or `format!("{}/bin/cargo", home)`.
#
# `$CARGO` (which cargo itself sets for child processes, e.g. for build
# scripts) is deliberately NOT flagged: it names whatever cargo invoked the
# current process -- if that invocation went through the broker, `$CARGO`
# *is* the broker. Using it does not bypass anything; it is the recommended
# way for a build script to find "the cargo that is running me".
#
# Known coverage gap (text-pattern lints cannot see everything): a bypass
# built by concatenating dynamic pieces across separate statements -- e.g.
# assigning `"cargo"` (or `"cargo.exe"`) to a variable on one line and a
# directory to another, then joining/formatting them together several
# lines later with no single line carrying enough context -- can still slip
# past every case above. Case 3/4 catch the common single-statement and
# single-call forms (including chains split one-`.join()`-per-line, since
# each matched call itself carries the literal), but a chain deliberately
# spread across unrelated statements is a real, acknowledged blind spot.
# Treat this lint as raising confidence, not proving absence, of a bypass.
#
# An inline marker `broker-bypass-lint:allow-line` on (or continuing into)
# a matched line exempts that one line without a checked-in allowlist
# entry -- e.g. for a workflow or doc comment that intentionally shows a
# bypass pattern as an example rather than executing one. This is the
# `.github/workflows` analog of this script excluding its own path below:
# a file doesn't have to be a script to document a bypass shape.
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
INLINE_MARKER='broker-bypass-lint:allow-line'

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

# Returns 0 if the given (already-1-indexed) line of the file carries the
# inline exemption marker. This is a lightweight per-line escape hatch for
# files (e.g. .github/workflows/*.yml comments, doc snippets) that must
# show a bypass pattern without triggering the lint and without needing a
# checked-in allowlist entry.
has_inline_marker() {
    marker_file=$1
    marker_line=$2
    sed -n "${marker_line}p" "$marker_file" 2>/dev/null | grep -qF "$INLINE_MARKER"
}

# --- Scan --------------------------------------------------------------------

# Case 1: explicit path to the rustup-proxy cargo binary (shell or Rust).
# shellcheck disable=SC2016 # single-quoted on purpose: this is a grep -E pattern, not a shell expansion
PATTERN_PATH='(\.cargo/bin/cargo(\.exe)?|\$CARGO_HOME/bin/cargo(\.exe)?)'
REASON_PATH='names the real cargo binary by path, bypassing the broker (case 1)'

# Case 3: Rust code building a resolved cargo-bin path via a `.join()` call
# naming the binary itself -- `cargo` or `cargo.exe` -- as the final path
# segment. Deliberately per-`.join()`-call (not per-chain): a chain split
# one call per line still matches on whichever line names the binary.
#
# Intentionally does NOT match `.join(".cargo")` alone: that call only
# reaches the `~/.cargo` directory (config, the `bin` dir itself, or some
# *other* binary installed there, e.g. `.join(".cargo").join("bin")
# .join(vox_binary_name())`), which several real, unrelated call sites do
# (config-file reads, `vox`/`voxup` binary-location helpers, per-crate
# script caches) -- none of them resolve cargo. Requiring the terminal
# `"cargo"`/`"cargo.exe"` segment is what makes this case 3, not case 1's
# broader literal-substring match.
# shellcheck disable=SC2016
PATTERN_JOIN_CARGO='\.join\([[:space:]]*"cargo(\.exe)?"[[:space:]]*\)'
REASON_JOIN='builds a resolved cargo-bin path via .join(), bypassing the broker (case 3)'

# Case 4a: a format!/string literal directly embedding "/bin/cargo(.exe)?".
# shellcheck disable=SC2016
PATTERN_FORMAT_EMBED='/bin/cargo(\.exe)?"'
# Case 4b: a two-placeholder template ("{}/bin/{}") paired on the same line
# with a "cargo"/"cargo.exe" argument -- e.g.
# `format!("{}/bin/{}", cargo_home, "cargo")`.
# shellcheck disable=SC2016
PATTERN_FORMAT_TEMPLATE='\{\}/bin/\{\}'
# shellcheck disable=SC2016
PATTERN_FORMAT_CARGO_ARG='"cargo(\.exe)?"'
REASON_FORMAT='assembles a cargo-bin path via format!/template, bypassing the broker (case 4)'

# detect_format_bypass CONTENT: returns 0 if the line matches case 4a or 4b.
detect_format_bypass() {
    fb_content=$1
    if printf '%s\n' "$fb_content" | grep -qE "$PATTERN_FORMAT_EMBED"; then
        return 0
    fi
    if printf '%s\n' "$fb_content" | grep -qE "$PATTERN_FORMAT_TEMPLATE" &&
        printf '%s\n' "$fb_content" | grep -qE "$PATTERN_FORMAT_CARGO_ARG"; then
        return 0
    fi
    return 1
}

# Case 2: `rustup run <toolchain> cargo ...` (resolves the toolchain cargo
# directly), and `rustup which cargo` (resolves + captures the real cargo
# path for a later call, e.g. `REAL_CARGO=$(rustup which cargo)`). Both
# patterns are evaluated by RUSTUP_JOIN_AWK below (awk's own regex engine,
# not a per-line grep -- see that comment for why), after joining a
# trailing `\` line continuation, so a wrapped `rustup run \` /
# `<toolchain> cargo ...` pair is still caught as one logical line.
REASON_RUSTUP_RUN='rustup run resolves the toolchain cargo directly, bypassing the broker (case 2)'
REASON_RUSTUP_WHICH='rustup which resolves + captures the real cargo path, bypassing the broker (case 2)'

# Joins `\`-continued lines, then tests both rustup patterns with awk's own
# regex engine and prints only the (rare) matching lines, tagged RUN/WHICH.
# This must NOT pipe every joined line of every scanned file through a
# shell loop (spawning `grep` per line), which is prohibitively slow across
# crates/**/*.rs (thousands of files, hundreds of thousands of lines).
# shellcheck disable=SC2016 # single-quoted on purpose: this is an awk script, not a shell expansion
RUSTUP_JOIN_AWK='
{
    startline = NR
    buf = $0
    while (buf ~ /\\[ \t]*$/) {
        if ((getline nxt) <= 0) { break }
        sub(/\\[ \t]*$/, " ", buf)
        buf = buf nxt
    }
    if (buf ~ /rustup[ \t]+run[ \t]+[^ \t]+[ \t]+cargo([ \t]|$)/) {
        print startline "\tRUN"
    }
    if (buf ~ /rustup[ \t]+which[ \t]+cargo([ \t]|$|[")])/) {
        print startline "\tWHICH"
    }
}
'

if [ -s "$FILES_TMP" ]; then
    sort -u "$FILES_TMP" >"$FILES_TMP.sorted"
    mv "$FILES_TMP.sorted" "$FILES_TMP"
    while IFS= read -r f; do
        [ -n "$f" ] || continue
        [ -f "$f" ] || continue

        # Cases 1, 3, 4: single-line patterns, scanned directly per line.
        grep -nE "$PATTERN_PATH" "$f" 2>/dev/null | while IFS=: read -r ln _; do
            has_inline_marker "$f" "$ln" || printf '%s:%s:%s\n' "$f" "$ln" "$REASON_PATH" >>"$MATCHES_TMP"
        done
        grep -nE "$PATTERN_JOIN_CARGO" "$f" 2>/dev/null | while IFS=: read -r ln _; do
            has_inline_marker "$f" "$ln" || printf '%s:%s:%s\n' "$f" "$ln" "$REASON_JOIN" >>"$MATCHES_TMP"
        done
        # shellcheck disable=SC2016 # single-quoted on purpose: a grep -E pattern, not a shell expansion
        grep -nE '(\{\}/bin/\{\}|/bin/cargo(\.exe)?")' "$f" 2>/dev/null | while IFS=: read -r ln rest; do
            if detect_format_bypass "$rest"; then
                has_inline_marker "$f" "$ln" || printf '%s:%s:%s\n' "$f" "$ln" "$REASON_FORMAT" >>"$MATCHES_TMP"
            fi
        done

        # Case 2: joined across `\` line continuations first; awk itself
        # decides match/no-match (see RUSTUP_JOIN_AWK above), so this loop
        # only ever iterates over the rare actual matches.
        awk "$RUSTUP_JOIN_AWK" "$f" 2>/dev/null | while IFS="$(printf '\t')" read -r ln tag; do
            [ -n "$ln" ] || continue
            case "$tag" in
                RUN)
                    has_inline_marker "$f" "$ln" || printf '%s:%s:%s\n' "$f" "$ln" "$REASON_RUSTUP_RUN" >>"$MATCHES_TMP"
                    ;;
                WHICH)
                    has_inline_marker "$f" "$ln" || printf '%s:%s:%s\n' "$f" "$ln" "$REASON_RUSTUP_WHICH" >>"$MATCHES_TMP"
                    ;;
            esac
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
