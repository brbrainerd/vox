#!/bin/sh
# Fixture for scripts/broker-bypass-lint.vox's self-test.
#
# This file is NOT excluded from the lint's default scan by accident -- it
# is excluded on purpose (see broker-bypass-lint.vox's fixtures-dir skip)
# because it deliberately contains a real bypass shape, so it must only be
# scanned when passed explicitly:
#
#   vox run scripts/broker-bypass-lint.vox -- scripts/broker-bypass-fixtures/known-bypass-rustup-which.sh
#
# and must make the lint exit non-zero with a "path:line:" report line.
set -eu

# Case 2 bypass: captures the resolved toolchain cargo path and invokes it
# directly -- functionally identical to the "run a toolchain's cargo
# directly" shape already covered above, phrased as a capture-then-call.
REAL_CARGO=$(rustup which cargo)
"$REAL_CARGO" build
