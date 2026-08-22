#!/bin/sh
# NOTE: This file is kept in sync with docs-astro/public/voxup by the
# documented_install_urls_are_served test. Both must be identical.
# Supported release targets (kept in sync with SUPPORTED_RELEASE_TARGETS in vox-cli):
#   x86_64-unknown-linux-gnu
#   x86_64-pc-windows-msvc
#   x86_64-apple-darwin
#   aarch64-apple-darwin
# voxup installer — macOS and Linux
# Usage (production):
#   curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
# Usage (local dev):
#   sh scripts/install.sh
set -eu

GITHUB_API="https://api.github.com/repos/vox-foundation/vox/releases/latest"
GITHUB_DL="https://github.com/vox-foundation/vox/releases/download"

# ── Helpers ─────────────────────────────────────────────────────────────────

say()      { printf "voxup: %s\n" "$*" >&2; }
err()      { say "error: $*"; exit 1; }
need_cmd() { command -v "$1" >/dev/null 2>&1 || err "need '$1' but it was not found in PATH"; }

# ── Platform detection ───────────────────────────────────────────────────────

detect_target() {
    _os="$(uname -s)"
    _arch="$(uname -m)"

    case "$_os" in
        Linux)  ;;
        Darwin) ;;
        *)      err "Unsupported OS: $_os (expected Linux or Darwin)" ;;
    esac

    case "$_arch" in
        x86_64)          ;;
        aarch64|arm64)   _arch="aarch64" ;;
        *)               err "Unsupported architecture: $_arch" ;;
    esac

    if [ "$_os" = "Linux" ]; then
        printf "%s" "${_arch}-unknown-linux-gnu"
    else
        printf "%s" "${_arch}-apple-darwin"
    fi
}

# ── SHA-256 verification ─────────────────────────────────────────────────────

verify_checksum() {
    _file="$1"
    _expected="$2"

    # Fail CLOSED. A missing hashing tool must abort, never downgrade to
    # installing unverified bytes — this runs inside `curl | sh`, where a
    # printed warning scrolls past unread.
    if command -v sha256sum >/dev/null 2>&1; then
        _actual="$(sha256sum "$_file" | cut -d ' ' -f1)"
    elif command -v shasum >/dev/null 2>&1; then
        _actual="$(shasum -a 256 "$_file" | cut -d ' ' -f1)"
    elif command -v openssl >/dev/null 2>&1; then
        # OpenSSL 3.x prints "SHA2-256(f)= <hex>", 1.x "SHA256(f)= <hex>",
        # LibreSSL "SHA256 (f) = <hex>". The hex is the last field in all three.
        _actual="$(openssl dgst -sha256 "$_file" | awk '{print $NF}')"
    else
        err "no SHA-256 tool found (need one of: sha256sum, shasum, openssl)."
    fi

    if [ "$_actual" != "$_expected" ]; then
        err "SHA-256 mismatch for $_file (expected $_expected, got $_actual)"
    fi
    say "Checksum OK"
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
    need_cmd curl
    need_cmd tar

    say "Detecting platform..."
    _target="$(detect_target)"
    say "Target: $_target"

    say "Fetching latest release info..."
    _tag="$(curl -sSfL \
        -H "Accept: application/vnd.github+json" \
        -H "User-Agent: voxup-install.sh" \
        "$GITHUB_API" \
        | grep '"tag_name"' \
        | head -1 \
        | sed 's/.*"tag_name": *"\(.*\)".*/\1/')"
    [ -n "$_tag" ] || err "Could not determine latest release tag from GitHub API"
    say "Latest release: $_tag"

    _archive="voxup-${_tag}-${_target}.tar.gz"
    _archive_url="${GITHUB_DL}/${_tag}/${_archive}"
    _checksums_url="${GITHUB_DL}/${_tag}/checksums.txt"

    _tmpdir="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$_tmpdir'" EXIT

    say "Downloading $_archive..."
    curl --proto '=https' --tlsv1.2 -sSfL "$_archive_url" -o "$_tmpdir/$_archive"

    say "Downloading checksums.txt..."
    curl --proto '=https' --tlsv1.2 -sSfL "$_checksums_url" -o "$_tmpdir/checksums.txt"

    _checksum="$(grep "  ${_archive}$" "$_tmpdir/checksums.txt" | cut -d ' ' -f1)"
    [ -n "$_checksum" ] || err "No checksum found for '$_archive' in checksums.txt"

    verify_checksum "$_tmpdir/$_archive" "$_checksum"

    say "Extracting..."
    tar -xzf "$_tmpdir/$_archive" -C "$_tmpdir"

    [ -f "$_tmpdir/voxup" ] || err "voxup binary not found after extraction"
    chmod +x "$_tmpdir/voxup"

    say "Running: voxup install default"
    "$_tmpdir/voxup" install default
}

main "$@"
