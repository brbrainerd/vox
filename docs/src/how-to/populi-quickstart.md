---
title: "Populi Quickstart"
description: "Run a local Vox mesh control plane and pair a worker node in minutes — no environment variables required."
category: "How-To Guides"
last_updated: "2026-05-01"
training_eligible: true
---

# Populi Quickstart

This guide walks you through running a local Vox mesh control plane — no environment variables required.

## Prerequisites

- `vox-ml-cli` built with the `populi` feature: `cargo build -p vox-ml-cli --features populi`
- A writable `~/.vox/` directory (created automatically on first run)

## Step 1 — Start the control plane

```sh
vox populi serve --enable
```

On the very first run, Vox generates a random bearer token and saves it to `~/.vox/config.toml` under the key `mesh.token`.  The token is printed once — copy it somewhere safe:

```text
vox populi: generated mesh bearer token (saved to ~/.vox/config.toml):
  VOX_MESH_TOKEN=a3f7c2...  ← copy this
  Keep this secret — it authenticates all control-plane requests.
vox populi: listening on http://127.0.0.1:PORT
```

The OS assigns a free port automatically (you can override with `--bind 127.0.0.1:9847`).

Subsequent runs reuse the saved token — no output unless it has changed.

## Step 2 — Verify the server is running

In a second terminal, set the token and probe the health endpoint:

```sh
curl http://127.0.0.1:PORT/health
# {"status":"ok"}

curl -H "Authorization: Bearer $VOX_MESH_TOKEN" \
     http://127.0.0.1:PORT/v1/populi/nodes
# {"nodes":[]}
```

Replace `PORT` with the port printed in Step 1, and `$VOX_MESH_TOKEN` with your token.

## Step 3 — Register a worker node

```sh
curl -s -X POST \
  -H "Authorization: Bearer $VOX_MESH_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"id":"my-node","scope_id":null,"capabilities":{"cpu_cores":8},"labels":{}}' \
  http://127.0.0.1:PORT/v1/populi/join | jq .
```

List nodes again to confirm:

```sh
curl -H "Authorization: Bearer $VOX_MESH_TOKEN" \
     http://127.0.0.1:PORT/v1/populi/nodes | jq .nodes[].id
# "my-node"
```

## Inspecting configuration

```sh
# Print all resolved config values and their sources
vox populi config show

# Validate the config and flag any missing or conflicting values
vox populi config check
```

## Pinning the port

Add `--bind 127.0.0.1:9847` to use a fixed port:

```sh
vox populi serve --enable --bind 127.0.0.1:9847
```

## Using the durable store (optional)

When the canonical VoxDb database is reachable, the control plane automatically uses it as a durable backing store for the A2A inbox, exec leases, and dispatch results.  The in-memory cache is warmed from the DB at startup.  No extra flags are needed — it just works.

## Token management

| Source | How |
|--------|-----|
| Auto-generated (default) | Saved to `~/.vox/config.toml` as `mesh.token` on first `--enable` run |
| Environment override | Set `VOX_MESH_TOKEN=<value>` before starting; takes precedence over config file |
| Manual set | `vox config set mesh.token <value>` (if `vox config` is available) |

To rotate the token, delete `mesh.token` from `~/.vox/config.toml` and restart with `--enable`.

## Connecting the orchestrator

Point the orchestrator at the control plane:

```sh
export VOX_MESH_CONTROL_ADDR=http://127.0.0.1:PORT
export VOX_MESH_TOKEN=<your-token>
vox orchestrate ...
```

## Troubleshooting

**Port already in use** — omit `--bind` to let the OS assign a free port, or choose a different port with `--bind 127.0.0.1:<PORT>`.

**401 Unauthorized** — the `Authorization: Bearer` header is missing or the token does not match the one saved in `~/.vox/config.toml`.  Run `vox populi config show` to check the token source.

**Mesh store warm-up warning** — `mesh store warm-up failed; continuing with empty cache` is printed when VoxDb is unavailable.  The server still starts and operates fully in-memory.
