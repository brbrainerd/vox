---
title: "ADR 047: iroh QUIC replaces the bespoke populi mesh transport"
description: "Adopts iroh 1.1 for mesh transport, identity, and NAT traversal, retiring the hand-rolled HTTP/JWT control plane. Records the measured evidence from the Task 0.2 spike."
category: "Architecture Decisions (ADRs)"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 047: iroh QUIC replaces the bespoke populi mesh transport

## Status

**Accepted (2026-09-04).**

(Numbering note: drafted as ADR-046, renumbered on merge with `origin/main`,
which had taken 046 for Pareto-frontier reporting. Commit messages and the plan
predating the merge still say 046.)

- **Supersedes** [ADR-008](008-populi-transport.md) (Mens transport) and
  [ADR-020](020-populi-mesh-scaling-transport-default.md) (default transport
  posture). ADR-020 §Decision.1 requires this ADR *by name* and pre-authorises
  the move by naming QUIC as the future option, so this is the ADR it asked for.
- **Partially supersedes** [ADR-017](017-populi-lease-remote-execution.md): the
  grant protocol only. **The duplicate-execution guard survives** — it is a
  scheduling invariant, not a transport concern.
- **Upholds** [ADR-018](018-populi-gpu-truth-layering.md). The GPU/CPU capability
  truth layer is the part vox actually contributes and is unchanged by this
  decision.

## Context

`vox populi`'s mesh has been an HTTP control plane with a hand-rolled ed25519
envelope and a JWT auth matrix, none of it going through `vox-crypto`. As of
`60d51c520` the daemon starts and `curl /health` returns 200 — but it binds
loopback, and **that bind is the only thing containing an unauthenticated
`FullAccess` path that writes posted bytes to a temp file, `chmod 0755`s them,
and executes them with no timeout** (fix F2 in the cross-machine handoff). There
is no cross-machine transport at all.

Building the missing half by hand means writing NAT traversal, hole punching,
path migration, key-based addressing, and a congestion-controlled stream
protocol. That work is precisely what iroh already vendors.

## Decision

Adopt **`iroh 1.1`** (pinned; `noq` QUIC, not quinn) as the mesh transport,
together with `iroh-tickets 1.0` for pairing and `iroh-mdns-address-lookup` for
LAN discovery. The `EndpointId` *is* the ed25519 public key, so transport and
identity are the same fact.

Non-negotiables carried from the design:

- **Never `presets::N0`, never `N0DisableRelay`, never `into_0rtt()`.** One
  detector, three patterns.
- **`presets::Minimal` only**, with `default-features = false`.
- **Trust is checked at accept *and* per request**, against
  `~/.vox/mesh_trust.json` keyed by `EndpointId` — not `trusted_nodes.json`,
  which is keyed by a `node_id` from a different keyspace.
- **Mesh-received work is sandboxed by default.** Pairing grants reachability,
  never native execution.
- **`iroh-blobs` and `irpc` are out of v1.** A bi-stream plus postcard is four
  lines; blobs earn their place for model and checkpoint distribution, not for
  10 MB payloads on a 3 ms LAN.

**Deletion happens last** — Phase 6, after the two-machine demo passes, and only
once the A2A mailbox, federation directory, queue stats, and `PopuliHttpOp` are
ported.

## Consequences

### Measured, not assumed

The Task 0.2 spike ran between a macOS and a Windows machine on `f48dbc810`;
full numbers in [iroh spike findings](../architecture/iroh-spike-findings-2026.md).

- **Zero third-party contact under `Minimal`.** `Builder::empty()` is documented
  as *"no address lookup services, and `RelayMode::Disabled`"*, and
  `Minimal::apply` sets only `crypto_provider`. `N0` is what adds pkarr, DNS
  lookup, and relays. The listener advertised IP addresses and no relay URL.
- **`default-features = false` also removes the portmapper**, so there is no
  UPnP/SSDP multicast and therefore no macOS firewall dialog. Recommended
  feature set: `["tls-ring", "fast-apple-datapath"]`.
- **Direct LAN connect in 12.8 ms** with no relay; 300 MiB transferred.
- **Placement telemetry is viable.** Differentiating `udp_tx.bytes` over 200 ms
  windows lands within **3.6 %** of wall-clock truth at a coefficient of
  variation of 14.6 % — ample for a roughly 2× "is shipping worth it" decision.
  `cwnd` is reachable only via `congestion_state()`, which documents itself as
  debug-only, so the bandwidth-delay-product shortcut is closed.
- **Cost: 55 marginal crates (+3.8 %)** and **71.95 s wall / 245.83 s user CPU**
  on a clean release build — an over-estimate, since it recompiles `tokio`,
  `serde`, `rustls` and `hyper` that the workspace already builds. This is inside
  the ~90 s threshold the operator's authorisation was conditional on.

### Accepted costs

- **Crate count goes up; hand-written security-critical code goes down.** Trading
  roughly 7 000 first-party lines — including the layer that produced the RCE —
  for vendored, tested ones makes the literal dependency metric worse and the
  real one better.
- **Two `ed25519-dalek` majors coexist.** `vox-crypto` pins 2.x, iroh resolves
  3.0.0. They cannot share a Rust type, but both round-trip through 32 raw bytes,
  so **one stored seed derives both identities**. Do not unify them: the mesh key
  must start headless while `NodeIdentity` is password-sealed, so fusing them
  either breaks unattended start or unseals the signing key.
- **`iroh-mdns-address-lookup` is pre-1.0** and must be pinned with `=`.

### Traps this ADR exists to record

- **`Endpoint::bind(preset)` takes no ALPNs.** A server built that way refuses
  every connection at ALPN negotiation — silently, not as a compile error. Use
  `Endpoint::builder(preset).alpns(...)`.
- **`ep.online().await` hangs forever under `Minimal`.** Its contract is "has
  contacted a relay server", and there is no relay. Never put it on this path.
- **`send.finish()` does not flush.** It signals end-of-stream; dropping the
  `Connection` immediately afterwards closes the connection before the bytes
  reach the wire, and the peer sees `closed by peer: 0` with no payload. The
  responder must await `conn.closed()` or equivalent. This cost the spike a
  debugging cycle and will recur in the real handler.
- **Set `NetReportConfig::minimal()` explicitly.** Its defaults enable HTTPS
  latency probes and captive-portal checks; those are inert today only because
  the relay map is empty.

## Alternatives rejected

- **Keep the HTTP plane and add TLS + real auth.** Does not address NAT traversal
  or hole punching, which is the actual missing capability, and leaves the
  hand-rolled envelope and JWT matrix in place.
- **Depend on Tailscale.** Rejected: the design must contact no third party, and
  the mesh has to work with both machines off the internet.
- **`libp2p`.** Heavier, and its QUIC story is a subset of what iroh provides
  with a worse identity model for this use case.
