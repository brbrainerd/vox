---
title: "Grand Network Quickstart (Volunteer Compute)"
description: "How to join or host a federated Vox volunteer compute node using vox populi join and vox populi attest."
category: "How-To Guides"
status: "current"
---

# Grand Network Quickstart

The Vox grand volunteer network lets you share spare GPU compute with trusted
peers — and consume theirs — with no central server, no token economy, and no
SaaS dependency. Federation is identity-first: signed `PublicAttestationManifest`
files, Gist-hosted, verified by anyone.

## Anti-goals

Before you start, note what this is _not_:

- No blockchain, no token.
- No automatic data sharing. Discovery publishing is opt-in and off by default.
- No transitive trust. Your node only trusts peers you explicitly register.

---

## Prerequisites

- Vox CLI installed (`vox --version` prints a version).
- A running `vox populi up` node (see [Populi reference](../reference/populi.md)).
- A GitHub account (for Gist-based manifest hosting — optional but recommended).

---

## Step 1 — Generate your node identity

When you first run `vox populi up`, a Mesh Federation Identity (Ed25519 key
pair) is generated automatically:

```sh
vox populi up --mode lan --public-mesh
```

Check your public key:

```sh
vox populi identity show
```

---

## Step 2 — Publish your attestation manifest

The attestation manifest announces your node's capabilities and opt-in status
to potential peers. It is a signed JSON document you host yourself.

Preview the manifest without publishing:

```sh
vox populi attest publish --dry-run --task-kinds text_infer,image_gen
```

Publish to a target URL (e.g. a raw Gist URL):

```sh
vox populi attest publish \
  --task-kinds text_infer \
  --target-url https://gist.githubusercontent.com/<user>/<gist-id>/raw/vox-manifest.json
```

Share this URL with peers who want to join your mesh.

---

## Step 3 — Join a peer's mesh

When a peer shares their invite URL or manifest URL, join them with:

```sh
# Direct manifest URL (HTTPS)
vox populi join https://gist.githubusercontent.com/<peer>/<gist-id>/raw/vox-manifest.json

# Invite URL (with optional signature and label)
vox populi join "vox-mesh://invite?manifest=<url>&label=Alice%27s+Node"
```

Dry-run first to preview:

```sh
vox populi join <url> --dry-run
```

The peer's manifest URL is saved to `~/.vox/config.toml` under
`mesh.federation_peers.<node-id>`.

---

## Step 4 — Verify the peer is registered

```sh
vox populi federation list
```

---

## Step 5 — Enable opt-in discovery publishing (optional)

Discovery publishing lets your node broadcast utilisation telemetry to the
Atlas Findings index. It is **disabled by default**:

```toml
# ~/.vox/config.toml
[mesh.discovery_publishing]
enabled = false   # change to true to opt in
```

When enabled and built with `--features mesh-discovery-publish`, the populi
daemon aggregates `vox.workflow.*` telemetry into `ProviderAtlasFinding`
observations every 5 minutes and publishes them to your attestation URL.

---

## Troubleshooting

| Symptom | Likely cause |
|---------|-------------|
| `VoxMeshFederationSigningKey not configured` | Run `vox populi up` first to generate the key. |
| `fetch manifest ... returned 404` | Check the manifest URL is publicly accessible. |
| `attestation manifest: no --target-url provided` | Pass `--target-url` or use `--dry-run`. |

---

## See also

- [Populi reference](../reference/populi.md)
- [Mesh and language distribution SSOT](../architecture/mesh-and-language-distribution-ssot-2026.md)
- `vox populi attest --help`
- `vox populi join --help`
