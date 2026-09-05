#!/bin/sh
# Fixture for scripts/broker-bypass-lint.vox's self-test.
#
# This file is NOT excluded from the lint's default scan by accident -- it
# is excluded on purpose (see broker-bypass-lint.vox's fixtures-dir skip)
# because it deliberately contains a real bypass shape, so it must only be
# scanned when passed explicitly:
#
#   vox run scripts/broker-bypass-lint.vox -- scripts/broker-bypass-fixtures/known-bypass-rustup-run-continuation.sh
#
# and must make the lint exit non-zero with a "path:line:" report line.
set -eu

# Case 2 bypass: resolving a toolchain's cargo directly, split across a
# backslash line continuation -- same runtime behavior as the single-line
# form, but spelled across two source lines so a naive per-line scan
# would miss it.
rustup run \
    stable cargo build --release
