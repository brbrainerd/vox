#!/bin/sh
# Fixture for scripts/broker-bypass-lint.vox's self-test.
#
# This file is NOT excluded from the lint's default scan by accident -- it is
# excluded on purpose (see broker-bypass-lint.vox's SELF_PATH/fixtures skip)
# because it deliberately contains a real bypass shape, so it must only be
# scanned when passed explicitly:
#
#   vox run scripts/broker-bypass-lint.vox -- scripts/broker-bypass-fixtures/known-bypass.sh
#
# and must make the lint exit non-zero with a "path:line:" report line.
set -eu

# Case 1 bypass: names the rustup-proxy cargo binary by explicit path
# instead of letting PATH resolution find the broker's `cargo` shim.
"$HOME/.cargo/bin/cargo" build --release
