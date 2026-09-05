#!/bin/sh
# Install and activate the vox build broker (a `cargo`-named shim that
# fair-queues cargo builds machine-wide). See:
#   docs/src/contributors/build-broker-usage.md
#
# Dry-run by default. Nothing is built, copied, or edited unless --apply is
# given. This installs a binary literally named `cargo` ahead of the rustup
# proxy on PATH -- a machine-wide change -- so the safe default is to show
# the plan and touch nothing.
#
# Usage:
#   scripts/broker-install.sh [--dry-run]
#   scripts/broker-install.sh --apply [--profile <path>] [--no-profile]
#
# Env:
#   VOX_BROKER_HOME   Broker install root. Default: $HOME/.vox/build-broker

set -eu

APPLY=0
NO_PROFILE=0
PROFILE_OVERRIDE=""
PROFILE_GIVEN=0

usage() {
    cat <<'EOF'
Usage: broker-install.sh [--dry-run]
       broker-install.sh --apply [--profile <path>] [--no-profile]

Install the vox build broker's cargo shim into $VOX_BROKER_HOME/bin
(default: $HOME/.vox/build-broker/bin) and, unless --no-profile is given,
prepend that directory to PATH in the detected login-shell profile.

Options:
  (none)            Same as --dry-run.
  --dry-run         Print the plan; change nothing on disk. Default.
  --apply           Build the shim, install it, and edit the shell profile.
  --profile <path>  Write the PATH block to this file instead of the
                     auto-detected login-shell profile. Required for testing.
  --no-profile      Install the binaries but do not touch any shell profile.
  --help            Show this help.

Env:
  VOX_BROKER_HOME   Broker install root (default: $HOME/.vox/build-broker).
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run)
            APPLY=0
            shift
            ;;
        --apply)
            APPLY=1
            shift
            ;;
        --profile)
            if [ $# -lt 2 ]; then
                echo "broker-install.sh: --profile requires a path argument" >&2
                exit 2
            fi
            PROFILE_OVERRIDE="$2"
            PROFILE_GIVEN=1
            shift 2
            ;;
        --no-profile)
            NO_PROFILE=1
            shift
            ;;
        --help | -h)
            usage
            exit 0
            ;;
        *)
            echo "broker-install.sh: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

# --- Locate the repo checkout this script lives in -------------------------
# Only needed for --apply (the build step). The self-installed copy under
# $VOX_BROKER_HOME/bin is for a later doctor check to *name*, not to re-run
# a build from -- it has no checkout beside it.
CDPATH=''
SCRIPT_DIR=$(cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/.." && pwd)
MANIFEST="$REPO_ROOT/crates/vox-cargo-shim/Cargo.toml"

BROKER_HOME=${VOX_BROKER_HOME:-"$HOME/.vox/build-broker"}
BIN_DIR="$BROKER_HOME/bin"
CARGO_BIN_DIR="$HOME/.cargo/bin"

START_MARK="# >>> vox build broker >>>"
END_MARK="# <<< vox build broker <<<"

# --- Decide which profile file we would touch -------------------------------
detect_shell_name() {
    case "${SHELL:-}" in
        */zsh) echo "zsh" ;;
        */bash) echo "bash" ;;
        */fish) echo "fish" ;;
        *) echo "unknown" ;;
    esac
}

PROFILE=""
PROFILE_KIND="posix"
if [ "$NO_PROFILE" -eq 0 ]; then
    if [ "$PROFILE_GIVEN" -eq 1 ]; then
        PROFILE="$PROFILE_OVERRIDE"
        case "$PROFILE" in
            */config.fish) PROFILE_KIND="fish" ;;
            *) PROFILE_KIND="posix" ;;
        esac
    else
        shell_name=$(detect_shell_name)
        case "$shell_name" in
            zsh)
                PROFILE="$HOME/.zshrc"
                PROFILE_KIND="posix"
                ;;
            bash)
                if [ "$(uname -s)" = "Darwin" ]; then
                    PROFILE="$HOME/.bash_profile"
                else
                    PROFILE="$HOME/.bashrc"
                fi
                PROFILE_KIND="posix"
                ;;
            fish)
                PROFILE="$HOME/.config/fish/config.fish"
                PROFILE_KIND="fish"
                ;;
            *)
                PROFILE="$HOME/.profile"
                PROFILE_KIND="posix"
                ;;
        esac
    fi
fi

if [ "$PROFILE_KIND" = "fish" ]; then
    PATH_LINE="set -gx PATH \"$BIN_DIR\" \$PATH"
else
    PATH_LINE="export PATH=\"$BIN_DIR:\$PATH\""
fi

# --- Dry run: print the plan, touch nothing --------------------------------
if [ "$APPLY" -eq 0 ]; then
    echo "vox build broker installer -- DRY RUN (no changes made)"
    echo
    echo "Would build:"
    echo "  cargo build --release --manifest-path $MANIFEST"
    echo
    echo "Would install into: $BIN_DIR"
    echo "  (every [[bin]] the vox-cargo-shim crate produces, e.g. cargo[.exe])"
    echo "  $BIN_DIR/broker-install.sh  (self-install, for later remediation)"
    echo
    if [ "$NO_PROFILE" -eq 1 ]; then
        echo "Would NOT edit any shell profile (--no-profile)."
    else
        echo "Would prepend to PATH ahead of $CARGO_BIN_DIR in: $PROFILE"
        echo "  $PATH_LINE"
        if [ ! -e "$PROFILE" ]; then
            echo "  (this file does not exist yet -- it would be created)"
        fi
    fi
    echo
    echo "Re-run with --apply to make these changes."
    exit 0
fi

# --- Apply -------------------------------------------------------------------

if [ ! -f "$MANIFEST" ]; then
    echo "broker-install.sh: cannot find $MANIFEST" >&2
    echo "broker-install.sh: run this from a vox repository checkout" >&2
    exit 1
fi

echo "Building vox-cargo-shim (release)..."
BUILD_JSON=$(mktemp)
trap 'rm -f "$BUILD_JSON"' EXIT

if ! (cd "$REPO_ROOT" && cargo build --release --manifest-path "$MANIFEST" --message-format=json) >"$BUILD_JSON"; then
    echo "broker-install.sh: build failed" >&2
    exit 1
fi

BIN_PATHS=$(grep -o '"executable":"[^"]*"' "$BUILD_JSON" | sed -E 's/^"executable":"(.*)"$/\1/' | sort -u)
rm -f "$BUILD_JSON"
trap - EXIT

if [ -z "$BIN_PATHS" ]; then
    echo "broker-install.sh: build produced no binaries" >&2
    exit 1
fi

mkdir -p "$BIN_DIR"

echo "$BIN_PATHS" | while IFS= read -r bin_path; do
    [ -n "$bin_path" ] || continue
    bin_name=$(basename "$bin_path")
    cp -p "$bin_path" "$BIN_DIR/$bin_name"
    echo "Installed $BIN_DIR/$bin_name"
done

cp -p "$SCRIPT_DIR/broker-install.sh" "$BIN_DIR/broker-install.sh"
chmod +x "$BIN_DIR/broker-install.sh"
echo "Installed $BIN_DIR/broker-install.sh (self-install copy)"

if [ "$NO_PROFILE" -eq 1 ]; then
    echo
    echo "--no-profile given: no shell profile was edited."
    echo "Add $BIN_DIR to PATH ahead of $CARGO_BIN_DIR yourself."
else
    profile_dir=$(dirname -- "$PROFILE")
    mkdir -p "$profile_dir"
    created=0
    if [ ! -e "$PROFILE" ]; then
        : >"$PROFILE"
        created=1
    fi

    STRIPPED=$(mktemp)
    awk -v start="$START_MARK" -v end="$END_MARK" '
        $0 == start { skip = 1; next }
        $0 == end   { skip = 0; next }
        skip != 1   { print }
    ' "$PROFILE" >"$STRIPPED"

    NEW_PROFILE=$(mktemp)
    cat "$STRIPPED" >"$NEW_PROFILE"
    # Ensure the block starts on its own line even if the stripped file has
    # no trailing newline.
    if [ -s "$STRIPPED" ]; then
        last_char=$(tail -c 1 "$STRIPPED" 2>/dev/null || true)
        if [ -n "$last_char" ]; then
            printf '\n' >>"$NEW_PROFILE"
        fi
    fi
    {
        echo "$START_MARK"
        echo "$PATH_LINE"
        echo "$END_MARK"
    } >>"$NEW_PROFILE"

    mv "$NEW_PROFILE" "$PROFILE"
    rm -f "$STRIPPED"

    echo
    if [ "$created" -eq 1 ]; then
        echo "Created $PROFILE (it did not exist)."
    fi
    echo "Edited $PROFILE:"
    echo "  $PATH_LINE"
fi

echo
echo "A running shell/IDE keeps its launch-time PATH -- open a new terminal" \
    "(or reload the IDE window) for the shim to take effect there."
