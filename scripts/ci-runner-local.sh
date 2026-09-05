#!/usr/bin/env bash
# Run local, EPHEMERAL self-hosted GitHub Actions runner(s) for this repo.
#
# WHY EPHEMERAL
# -------------
# vox-foundation/vox is a PUBLIC repository. GitHub's own guidance is not to
# attach self-hosted runners to public repos, because anyone who can open a
# pull request can cause a workflow to execute code on the runner host. This
# script does not remove that risk; it bounds it:
#
#   * --ephemeral: each runner takes exactly ONE job, then deregisters and its
#     container exits. Nothing a job leaves behind survives into the next job.
#   * Each container mounts NO host filesystem except a named Docker volume
#     for the cargo/sccache caches, and NOT the Docker socket. A job cannot
#     reach your home directory, your keychain, or the host daemon.
#   * A fresh registration token is minted per iteration, per runner. Tokens
#     live ~1 hour, so a long-running loop cannot be resumed with a stale
#     credential.
#
# You should ALSO set, in the repo's Actions settings:
#   Settings -> Actions -> General -> Fork pull request workflows from
#   outside collaborators -> "Require approval for all external contributors"
# That setting is UI-only (no REST endpoint), so this script cannot set it or
# verify it for you.
#
# CONCURRENCY (RUNNER_COUNT)
# ---------------------------
# The repo normally has exactly one registered runner, so concurrent workflow
# runs queue behind each other and a slow one can get cancelled by the next
# push. RUNNER_COUNT supervises that many independent ephemeral-runner loops
# side by side, each with its own container name, token, and lifecycle — the
# per-job security properties above are unchanged, just replicated N times.
#
# It defaults to 1 (identical to the old single-runner behavior) and is
# capped at 8: the host has 18 cores and colima's VM caps out at 12, amd64
# runners run under Rosetta emulation (heavier per runner than native), and
# 8 leaves headroom for the host OS, Docker itself, and whatever else is
# running instead of assuming the whole colima budget is free for CI. Raise
# the cap only if you've checked the machine can actually take it.
#
# USAGE
#   scripts/ci-runner-local.sh                  # serve jobs until Ctrl-C
#   RUNNER_ONCE=1 scripts/ci-runner-local.sh    # each runner serves one job, stops
#   RUNNER_COUNT=4 scripts/ci-runner-local.sh   # supervise 4 concurrent runners
#
# RUNNER_ONCE + RUNNER_COUNT compose per-worker: with RUNNER_COUNT=N, each of
# the N workers stops after its own first job, so the whole script serves at
# most N jobs total (fewer if Ctrl-C'd first) and then exits.
#
# Requires: docker, gh (authenticated, `repo` scope), and the image:
#   docker build --platform linux/amd64 -t vox-ci-runner-local:amd64 \
#     -f infra/ci-runner/Dockerfile .
# (or ARCH=arm64 and --platform linux/arm64 — see the ARCH note below)
set -euo pipefail

REPO="${REPO:-vox-foundation/vox}"
CACHE_VOLUME="${CACHE_VOLUME:-vox-ci-cache}"

# How many concurrent ephemeral runners to supervise. See CONCURRENCY above.
RUNNER_COUNT="${RUNNER_COUNT:-1}"
MAX_RUNNER_COUNT=8
case "$RUNNER_COUNT" in
  ''|*[!0-9]*)
    echo "RUNNER_COUNT must be a positive integer (got: $RUNNER_COUNT)" >&2
    exit 1
    ;;
esac
if [ "$RUNNER_COUNT" -lt 1 ]; then
  echo "RUNNER_COUNT must be a positive integer (got: $RUNNER_COUNT)" >&2
  exit 1
fi
if [ "$RUNNER_COUNT" -gt "$MAX_RUNNER_COUNT" ]; then
  echo "RUNNER_COUNT=$RUNNER_COUNT exceeds the safety cap of $MAX_RUNNER_COUNT" \
    "(this spawns containers on your laptop — raise MAX_RUNNER_COUNT in the" \
    "script only after checking the host can take it)" >&2
  exit 1
fi

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

# Container names stay unique PER ITERATION (worker + iteration), not just per
# worker. `docker run --rm` returns as soon as the container exits, but the
# daemon removes it asynchronously -- reusing one fixed name per worker meant
# the next iteration could race that removal and die on
# `Conflict. The container name "..." is already in use`. Cleanup therefore
# finds containers by a run-scoped label instead of by name, which works
# regardless of which iteration is in flight.
RUN_LABEL="vox-runner-local-run=$$"
container_name() { echo "vox-runner-local-$$-$1-$2"; }

cleaned=0
worker_pids=()
cleanup() {
  # Idempotent: INT/TERM and the script's own EXIT can all reach this.
  [ "$cleaned" = "1" ] && return 0
  cleaned=1
  echo
  echo "runner: stopping ($RUNNER_COUNT worker(s))"
  for pid in "${worker_pids[@]:-}"; do
    [ -n "$pid" ] && kill "$pid" >/dev/null 2>&1 || true
  done
  # Label lookup, not name reconstruction: the iteration counter lives inside
  # each worker subshell, so the parent cannot know which name is live.
  local ids
  ids="$(docker ps -aq --filter "label=${RUN_LABEL}" 2>/dev/null || true)"
  if [ -n "$ids" ]; then
    # shellcheck disable=SC2086
    docker rm -f $ids >/dev/null 2>&1 || true
  fi
  wait >/dev/null 2>&1 || true
}
trap cleanup INT TERM EXIT

run_worker() {
  local worker="$1"
  local i=0
  while :; do
    i=$((i + 1))
    local name
    name="$(container_name "$worker" "$i")"
    # Fresh token per job: ephemeral runners re-register on every start, and a
    # registration token only lives ~1h.
    local token
    token="$(gh api -X POST "repos/${REPO}/actions/runners/registration-token" --jq .token)"
    [ -n "$token" ] || {
      echo "runner[$worker]: could not mint a registration token" >&2
      return 1
    }

    echo "runner[$worker]: waiting for a job (iteration $i)"
    docker run --rm --name "$name" --label "$RUN_LABEL" \
      -e REPO_URL="https://github.com/${REPO}" \
      -e RUNNER_TOKEN="$token" \
      -e RUNNER_LABELS="$RUNNER_LABELS" \
      -e RUNNER_NAME="vox-local-$(hostname -s)-${worker}-${i}" \
      -e RUNNER_EPHEMERAL=1 \
      -v "${CACHE_VOLUME}:/cache" \
      "$IMAGE" || echo "runner[$worker]: container exited non-zero (iteration $i)"

    if [ "${RUNNER_ONCE:-0}" = "1" ]; then
      echo "runner[$worker]: RUNNER_ONCE set, stopping"
      break
    fi
  done
}

echo "runner: repo=$REPO arch=$ARCH image=$IMAGE labels=$RUNNER_LABELS count=$RUNNER_COUNT (ephemeral)"
for w in $(seq 1 "$RUNNER_COUNT"); do
  run_worker "$w" &
  worker_pids+=("$!")
done
wait "${worker_pids[@]}"
