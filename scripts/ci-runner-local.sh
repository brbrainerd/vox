#!/usr/bin/env bash
# Run a local, EPHEMERAL self-hosted GitHub Actions runner for this repo.
#
# WHY EPHEMERAL
# -------------
# vox-foundation/vox is a PUBLIC repository. GitHub's own guidance is not to
# attach self-hosted runners to public repos, because anyone who can open a
# pull request can cause a workflow to execute code on the runner host. This
# script does not remove that risk; it bounds it:
#
#   * --ephemeral: the runner takes exactly ONE job, then deregisters and the
#     container exits. Nothing a job leaves behind survives into the next job.
#   * The container mounts NO host filesystem except a named Docker volume for
#     the cargo/sccache caches, and NOT the Docker socket. A job cannot reach
#     your home directory, your keychain, or the host daemon.
#   * A fresh registration token is minted per iteration. Tokens live ~1 hour,
#     so a long-running loop cannot be resumed with a stale credential.
#
# You should ALSO set, in the repo's Actions settings:
#   Settings -> Actions -> General -> Fork pull request workflows from
#   outside collaborators -> "Require approval for all external contributors"
# That setting is UI-only (no REST endpoint), so this script cannot set it or
# verify it for you.
#
# USAGE
#   scripts/ci-runner-local.sh                  # serve jobs until Ctrl-C
#   RUNNER_ONCE=1 scripts/ci-runner-local.sh    # serve exactly one job, stop
#
# Requires: docker, gh (authenticated, `repo` scope), and the image:
#   docker build --platform linux/amd64 -t vox-ci-runner-local:amd64 \
#     -f infra/ci-runner/Dockerfile .
# (or ARCH=arm64 and --platform linux/arm64 — see the ARCH note below)
set -euo pipefail

REPO="${REPO:-vox-foundation/vox}"
CACHE_VOLUME="${CACHE_VOLUME:-vox-ci-cache}"

# ARCH: fidelity vs speed. This is a real tradeoff, not a default to skip past.
#
#   amd64 (default) — matches GitHub-hosted runners and the release targets.
#     On Apple silicon this runs under colima's Rosetta. Slower, but it is the
#     same choice .actrc already makes for local runs, and for the same stated
#     reason: an arm64-only local lane lets jobs "pass locally against arm64
#     toolchains while CI runs amd64", which is a false green.
#
#   arm64 — native on Apple silicon and substantially faster, at the cost of
#     that fidelity. Defensible ONLY if x86_64 coverage is guaranteed
#     elsewhere; today that means cross-platform-check, which builds x86_64 on
#     GitHub-hosted runners with zero queue — but which is currently only
#     advisory, so the guarantee is not actually enforced yet.
#
# Set ARCH=arm64 to opt into speed. Labels follow the arch automatically so a
# runner never advertises an architecture it is not.
ARCH="${ARCH:-amd64}"
case "$ARCH" in
  amd64) DEFAULT_LABEL_ARCH="x64" ;;
  arm64) DEFAULT_LABEL_ARCH="arm64" ;;
  *) echo "ARCH must be amd64 or arm64 (got: $ARCH)" >&2; exit 1 ;;
esac
IMAGE="${IMAGE:-vox-ci-runner-local:$ARCH}"
RUNNER_LABELS="${RUNNER_LABELS:-self-hosted,linux,$DEFAULT_LABEL_ARCH}"

command -v docker >/dev/null || { echo "docker not found" >&2; exit 1; }
command -v gh >/dev/null || { echo "gh not found" >&2; exit 1; }
docker image inspect "$IMAGE" >/dev/null 2>&1 || {
  echo "image $IMAGE not found — build it first:" >&2
  echo "  docker build --platform linux/$ARCH -t $IMAGE -f infra/ci-runner/Dockerfile ." >&2
  exit 1
}
docker volume create "$CACHE_VOLUME" >/dev/null

name=""
cleanup() {
  echo
  echo "runner: stopping"
  [ -n "$name" ] && docker rm -f "$name" >/dev/null 2>&1 || true
  exit 0
}
trap cleanup INT TERM

echo "runner: repo=$REPO arch=$ARCH image=$IMAGE labels=$RUNNER_LABELS (ephemeral)"
i=0
while :; do
  i=$((i + 1))
  name="vox-runner-local-$$-$i"
  # Fresh token per job: ephemeral runners re-register on every start, and a
  # registration token only lives ~1h.
  token="$(gh api -X POST "repos/${REPO}/actions/runners/registration-token" --jq .token)"
  [ -n "$token" ] || {
    echo "runner: could not mint a registration token" >&2
    exit 1
  }

  echo "runner: waiting for a job (iteration $i)"
  docker run --rm --name "$name" \
    -e REPO_URL="https://github.com/${REPO}" \
    -e RUNNER_TOKEN="$token" \
    -e RUNNER_LABELS="$RUNNER_LABELS" \
    -e RUNNER_NAME="vox-local-$(hostname -s)-$i" \
    -e RUNNER_EPHEMERAL=1 \
    -v "${CACHE_VOLUME}:/cache" \
    "$IMAGE" || echo "runner: container exited non-zero (iteration $i)"

  if [ "${RUNNER_ONCE:-0}" = "1" ]; then
    echo "runner: RUNNER_ONCE set, stopping"
    break
  fi
done
