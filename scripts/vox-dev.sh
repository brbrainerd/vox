#!/usr/bin/env bash
# vox-dev.sh (Thin Launcher)
# Forward all arguments to vox-cli via cargo run.
set -euo pipefail
if command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER=sccache
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" && exec cargo run -q -p vox-cli -- "$@"
