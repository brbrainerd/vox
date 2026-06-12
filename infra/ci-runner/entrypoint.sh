#!/bin/bash
# Entrypoint for the vox self-hosted CI runner image (infra/ci-runner/Dockerfile).
#
# Supports two modes via RUNNER_EPHEMERAL:
#   0 (default) — persistent runner: configure once (`.runner` persists across
#                 container restarts), then run jobs forever. Legacy vox-runner-1/2.
#   1           — ephemeral runner: (re)configure with `--ephemeral` every start,
#                 run exactly ONE job, then the runner self-deregisters and exits.
#                 Used by the `vox ci runner-scale` autoscaler.
set -euo pipefail

cd /actions-runner

REPO_URL="${REPO_URL:?REPO_URL required}"
RUNNER_TOKEN="${RUNNER_TOKEN:?RUNNER_TOKEN required}"
RUNNER_LABELS="${RUNNER_LABELS:-self-hosted,linux,x64}"
RUNNER_NAME="${RUNNER_NAME:-vox-local}"
RUNNER_EPHEMERAL="${RUNNER_EPHEMERAL:-0}"

extra_args=()
if [ "${RUNNER_EPHEMERAL}" = "1" ]; then
  extra_args+=(--ephemeral)
fi

# Ephemeral runners reconfigure on every start (no persistent identity);
# persistent runners configure once.
# Persist cargo registry/git and advisory DB on the shared /cache volume (see runner_scale.rs).
mkdir -p /cache/sccache /cache/cargo-registry /cache/cargo-git /cache/advisory-db
if [ -n "${HOME:-}" ]; then
  mkdir -p "${HOME}/.cargo"
  ln -sfn /cache/cargo-registry "${HOME}/.cargo/registry"
  ln -sfn /cache/cargo-git "${HOME}/.cargo/git"
  ln -sfn /cache/advisory-db "${HOME}/.cargo/advisory-db"
fi

if [ "${RUNNER_EPHEMERAL}" = "1" ] || [ ! -f .runner ]; then
  ./config.sh \
    --url "${REPO_URL}" \
    --token "${RUNNER_TOKEN}" \
    --labels "${RUNNER_LABELS}" \
    --name "${RUNNER_NAME}" \
    --work _work \
    --unattended \
    --replace \
    ${extra_args[@]+"${extra_args[@]}"}
fi

# Best-effort deregister on graceful shutdown (persistent mode; ephemeral
# runners deregister themselves after their single job).
cleanup() {
  ./config.sh remove --token "${RUNNER_TOKEN}" >/dev/null 2>&1 || true
}
trap 'cleanup; exit 0' INT TERM

exec ./run.sh
