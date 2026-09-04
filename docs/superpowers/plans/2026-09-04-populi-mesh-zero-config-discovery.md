# Populi Mesh Ticket Pairing and LAN Discovery — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two machines with vox installed and no configuration exchange one pasteable ticket and share dispatched work — with no account, no internet, and no central server — plus automatic LAN discovery where the network permits it.

**Architecture:** A universal `vox-mesh://` ticket carrying a public key is the default pairing path (works on every network). mDNS discovery is a convenience layer over the same trust path. The control plane's auth default becomes fail-closed before any bind widens, so trust is enforced rather than decorative.

**Tech Stack:** Rust 1.96, `mdns-sd = "=0.20"` (new, pure-Rust, no daemon), axum control plane, Tauri 2 + React GUI, `vox_identity` ed25519 trust store.

**Spec:** [`docs/superpowers/specs/2026-09-04-populi-mesh-zero-config-discovery-design.md`](../specs/2026-09-04-populi-mesh-zero-config-discovery-design.md) — revision 2.

**Provenance:** Revision 2, rewritten after an eight-track adversarial audit found 24 confirmed defects in revision 1, including four false rationales and one security regression. Findings verified *correct* are in the appendix — do not relitigate them.

## Global Constraints

- **No new workspace crate edges.** `vox-orchestrator-mcp → vox-identity` and `vox-ml-cli → vox-identity` **do not exist** and must not be added (`AGENTS.md` §Dependency Discipline: exceptions are user-authorized only). Route through `vox-populi`, which already depends on `vox-identity`. If you think you need an edge, stop and ask.
- **Three processes.** `vox-gui` → TCP `127.0.0.1:9745` → `vox-orchestrator-d` (where `vox_mesh_nodes` executes) → and separately `vox populi serve`. No process-global spans them. Verify which process your code runs in before storing state in one.
- **Never bind `0.0.0.0`,** and never widen a bind before Phase 0's auth gate has landed.
- **Discovery is never fatal.** Any failure logs at `warn` and leaves the node working on T0/T1/T3.
- **Trust binds to the pubkey, never to a self-asserted `node_id`.** No code path may write a trust row with an empty `pubkey_hex`.
- **Test-first** (`AGENTS.md`): the detector is file-granular — every file with a `pub fn` needs at least one `#[test]` in that same file.
- **Formatting:** `vox run scripts/fmt.vox`. **Never** `cargo fmt --all` (Windows `os error 206`).
- **Pre-push:** `vox ci pre-push --complete`; the fast tier runs no clippy.
- Discovery must **not** ride the `populi-transport` feature — that gates HTTP machinery and is off in every shipped binary.

---

## Phase 0 — Security prerequisites (blocking)

Nothing in Phase 1+ may land first. Every Phase 0 task fixes a live defect in `main` and is independently valuable.

### Task 0.1: `vox populi up` actually starts a server

**Files:** Modify `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:178-190`

`up` spawns `populi serve --bind <addr>` with **no `--enable`**; `populi_cli.rs:850` bails immediately without it, and both pipes are `Stdio::null()`, so it dies silently while the parent records a pid and writes `mesh-state.json`. **`vox populi up` has never started a server.** Everything downstream is unverifiable until this is fixed.

- [ ] **Step 1: Reproduce**

```bash
cargo run -p vox-cli --features populi -- populi up --mode lan
```

Then `curl http://127.0.0.1:9847/health`. Expected: connection refused, despite `up` printing "Populi started" and a pid.

- [ ] **Step 2: Write the failing test**

```rust
/// Build the argv for the spawned `populi serve` child.
///
/// Extracted so the argument list is assertable without spawning anything —
/// the omission of `--enable` silently produced a dead daemon.
pub(crate) fn serve_child_args(bind: &str) -> Vec<String> {
    vec![
        "populi".to_string(),
        "serve".to_string(),
        "--enable".to_string(),
        "--bind".to_string(),
        bind.trim().to_string(),
    ]
}

#[cfg(test)]
mod serve_args_tests {
    use super::*;

    #[test]
    fn serve_child_is_passed_enable_or_it_exits_immediately() {
        assert!(
            serve_child_args("127.0.0.1:9847").contains(&"--enable".to_string()),
            "`populi serve` bails without --enable (populi_cli.rs:850)"
        );
    }

    #[test]
    fn bind_is_forwarded_verbatim_after_trimming() {
        let a = serve_child_args("  192.168.1.9:7000 ");
        assert_eq!(a[a.len() - 1], "192.168.1.9:7000");
    }
}
```

- [ ] **Step 3: Run to verify it fails**

`cargo test -p vox-ml-cli --features populi serve_args_tests`
Expected: FAIL — `serve_child_args` is not yet used by the spawn path (and the module does not exist).

- [ ] **Step 4: Use it, and stop swallowing stderr**

```rust
let exe = std::env::current_exe().context("resolve current executable path")?;
let log_path = mesh_dir.join("serve.log");
let log = std::fs::File::create(&log_path)
    .with_context(|| format!("create {}", log_path.display()))?;
let mut child = std::process::Command::new(exe);
child
    .args(serve_child_args(&effective_bind))
    .stdout(Stdio::null())
    .stderr(Stdio::from(log));
```

- [ ] **Step 5: Add a post-spawn health gate**

Before printing "Populi started", poll `control_plane_health(&control_url)` for up to 5 seconds. On failure, print the tail of `serve.log` and return an error rather than writing `mesh-state.json`.

- [ ] **Step 6: Verify manually**

`vox populi up --mode lan`, then `curl http://127.0.0.1:9847/health` → success. Then `vox populi down`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-ml-cli/
git commit -m "fix(populi): pass --enable so populi up actually starts the server"
```

### Task 0.2: Fail-closed auth when the bind is not loopback

**Files:** Modify `crates/vox-plugin-populi-mesh/src/transport/{router.rs,auth.rs}`

The gate that makes every later task safe. See spec §5: with no auth material configured, `requires_bearer()` is false, every route gets `FullAccess`, and that is the only gate on `POST /v1/populi/worker/execute` — which writes posted bytes to the temp dir and executes them.

- [ ] **Step 1: Write the failing test**

```rust
/// Whether an unauthenticated request may be granted `FullAccess`.
///
/// Historically `!requires_bearer()` granted it unconditionally, which on a
/// non-loopback bind exposes `/v1/populi/worker/execute` — a write-then-exec
/// endpoint — to anyone who can reach the port. Absent credentials are only
/// safe on loopback.
#[must_use]
pub fn open_access_permitted(requires_bearer: bool, bind_is_loopback: bool) -> bool {
    !requires_bearer && bind_is_loopback
}

#[cfg(test)]
mod open_access_tests {
    use super::*;

    #[test]
    fn no_auth_on_loopback_stays_open_for_single_machine_use() {
        assert!(open_access_permitted(false, true));
    }

    #[test]
    fn no_auth_off_loopback_is_denied() {
        assert!(
            !open_access_permitted(false, false),
            "unauthenticated LAN access reaches worker/execute"
        );
    }

    #[test]
    fn configured_auth_never_takes_the_open_path() {
        assert!(!open_access_permitted(true, true));
        assert!(!open_access_permitted(true, false));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

`cargo test -p vox-plugin-populi-mesh open_access_tests`
Expected: FAIL — `open_access_permitted` not defined.

- [ ] **Step 3: Run to verify it passes** — implementation is in Step 1. PASS, 3 tests.

- [ ] **Step 4: Wire it into the middleware**

`PopuliTransportState` records whether its listener is loopback at bind time. In `router.rs`, replace `if !runtime.requires_bearer()` with `if open_access_permitted(runtime.requires_bearer(), state.bind_is_loopback)`. When false and no valid bearer or node signature is presented, return `401` naming the fix (set `VOX_MESH_TOKEN`, or trust this node's key).

- [ ] **Step 5: Wire the ed25519 trust verifier**

`auth_ed25519::verify_against_trust` has zero non-test callers. Call it from the middleware's signature branch, so a signed request from a key in `TrustedNodeRegistry` authenticates without a bearer. This is the zero-config auth path, and it depends on Task 0.4's `lookup_by_pubkey_hex` fix.

- [ ] **Step 6: Extend the signed payload to cover the body**

The signature currently covers `path.ts.nonce` only, so it authenticates the sender but not the payload — an active MITM can swap the body of a signed `worker/execute`. Include a body hash.

- [ ] **Step 7: Add the integration test that matters**

```rust
#[tokio::test]
async fn unauthenticated_worker_execute_is_refused_on_a_non_loopback_bind() {
    let app = populi_http_app(state_bound_to("192.168.1.5:0"));
    let res = post(&app, "/v1/populi/worker/execute", r#"{"source":"","is_bundle":false}"#).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 8: Commit**

```bash
git commit -m "fix(populi): deny unauthenticated access on non-loopback binds"
```

### Task 0.3: `--insecure-local` refuses a non-loopback bind

**Files:** Modify `crates/vox-ml-cli/src/commands/populi_lifecycle.rs:117-130`

- [ ] **Step 1: Write the failing test**

```rust
/// `--insecure-local` disables the bearer requirement. Defensible on loopback;
/// off-loopback it publishes an open control plane to the network.
pub(crate) fn insecure_local_allowed(bind: &str) -> bool {
    let b = bind.trim();
    b.starts_with("127.") || b.starts_with("[::1]")
}

#[cfg(test)]
mod insecure_tests {
    use super::*;
    #[test]
    fn allowed_on_loopback() { assert!(insecure_local_allowed("127.0.0.1:9847")); }
    #[test]
    fn refused_on_a_lan_address() { assert!(!insecure_local_allowed("192.168.1.9:9847")); }
    #[test]
    fn refused_on_a_wildcard() { assert!(!insecure_local_allowed("0.0.0.0:9847")); }
}
```

- [ ] **Step 2: Run to verify it fails, then passes**

`cargo test -p vox-ml-cli --features populi insecure_tests`

- [ ] **Step 3: Hard-fail in `up`**

When `--insecure-local` is set and `insecure_local_allowed(&effective_bind)` is false, `anyhow::bail!` naming the resolved bind.

- [ ] **Step 4: Fix the inherited-env hole**

`env_map.remove("VOX_MESH_TOKEN")` only clears the map; `Command` inherits the parent environment, so an exported token survives and the flag is not honored either way. Add `.env_remove("VOX_MESH_TOKEN")` on the child.

- [ ] **Step 5: Commit**

### Task 0.4: Three correctness fixes in the trust and bootstrap paths

**Files:** `crates/vox-plugin-populi-mesh/src/transport/handlers/nodes.rs`, `crates/vox-identity/src/trust.rs`

- [ ] **Step 1: Bootstrap token — compare before burning**

`bootstrap_used.swap(true, SeqCst)` runs *before* the token comparison, so one unauthenticated malformed POST permanently consumes the one-shot window. Move the swap after `bearer_token_eq`.

```rust
#[tokio::test]
async fn a_wrong_bootstrap_token_does_not_burn_the_window() {
    let st = state_with_bootstrap("correct-token");
    let _ = bootstrap_exchange(&st, "wrong-token").await;
    assert!(bootstrap_exchange(&st, "correct-token").await.is_ok(),
        "a failed attempt must not consume the one-shot window");
}
```

- [ ] **Step 2: `lookup_by_pubkey_hex` must read disk**

It searches only `self.cache`, always empty for a file-backed registry, so it always returns `None` — and it is what Task 0.2's verifier depends on.

```rust
pub fn lookup_by_pubkey_hex(&self, pubkey_hex: &str) -> Option<TrustedNode> {
    if pubkey_hex.is_empty() {
        return None; // an empty key must never match, including empty stored rows
    }
    if let Some(n) = self.cache.values().find(|n| n.pubkey_hex == pubkey_hex) {
        return Some(n.clone());
    }
    self.load().ok()?.into_values().find(|n| n.pubkey_hex == pubkey_hex)
}
```

- [ ] **Step 3: Add a file-backed constructor**

`new_in_memory()` silently discards writes (`save()` returns `Ok(())` without storing), so every test using it asserts nothing about persistence — a trap for every task in this plan.

```rust
/// File-backed registry at an explicit path. Lets tests exercise real
/// persistence; `new_in_memory()` discards writes silently.
pub fn at(path: PathBuf) -> Self {
    Self { path: Some(path), cache: HashMap::new() }
}
```

- [ ] **Step 4: Test both against a real temp file**

```rust
#[test]
fn lookup_by_pubkey_reads_the_file_not_just_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.json");
    TrustedNodeRegistry::at(path.clone()).add("n1".into(), "deadbeef".into(), None).unwrap();
    // A fresh handle: cache is empty, so this can only pass by loading.
    assert!(TrustedNodeRegistry::at(path).lookup_by_pubkey_hex("deadbeef").is_some());
}

#[test]
fn an_empty_pubkey_never_matches() {
    let dir = tempfile::tempdir().unwrap();
    let reg = TrustedNodeRegistry::at(dir.path().join("t.json"));
    reg.add("legacy".into(), String::new(), None).unwrap();
    assert!(reg.lookup_by_pubkey_hex("").is_none());
}
```

- [ ] **Step 5: Commit**

---

## Phase 1 — The ticket (T1), the universal path

### Task 1.1: Ticket format

**Files:** Create `crates/vox-populi/src/discovery/{mod.rs,ticket.rs}`; modify `crates/vox-populi/src/lib.rs`, `Cargo.toml`

**Produces:** `Ticket { node_id, host, port, pubkey_hex }`, `Ticket::parse`, `Ticket::render`, `Ticket::control_url`, `node_id_for_pubkey`.

No `mdns-sd` dependency — this tier has none.

- [ ] **Step 1: Add the features**

```toml
# Peer pairing: tickets (always) and mDNS LAN discovery (with `discovery-mdns`).
# Deliberately NOT part of `transport` — that gates HTTP machinery which is off
# in every shipped binary, and pairing must work without it.
discovery = []
discovery-mdns = ["discovery", "dep:mdns-sd"]
```

Add `discovery` to `default`. Requirement 1 cannot be met by an opt-in feature.

- [ ] **Step 2: Write the failing test**

```rust
//! `vox-mesh://` pairing tickets — the universal pairing path (spec §3.1).
//!
//! Works on every network, including managed Windows, guest Wi-Fi with client
//! isolation, and across VLANs, where multicast discovery cannot.

use std::fmt;

/// A pairing ticket. Carries a *public* key: it is not a bearer secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub node_id: String,
    pub host: String,
    pub port: u16,
    pub pubkey_hex: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TicketError {
    Scheme,
    Malformed,
    BadKey,
    /// The node id does not match the key. Tickets are self-verifying; this
    /// means the ticket was altered or mistyped.
    IdKeyMismatch,
}

impl fmt::Display for TicketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Scheme => "not a vox-mesh:// ticket",
            Self::Malformed => "malformed ticket (expected vox-mesh://<id>@<host>:<port>#<pubkey>)",
            Self::BadKey => "public key must be 64 hex characters",
            Self::IdKeyMismatch => "node id does not match the public key",
        })
    }
}

/// Node id derived from a public key. MUST match `vox auth trust`'s derivation
/// (crates/vox-cli/src/commands/auth.rs) or tickets produce untrustable rows.
pub fn node_id_for_pubkey(pubkey_hex: &str) -> Option<String> {
    let bytes = hex::decode(pubkey_hex).ok()?;
    Some(hex::encode(&vox_crypto::facades::secure_hash(&bytes)[..8]))
}

impl Ticket {
    pub fn parse(raw: &str) -> Result<Self, TicketError> {
        let rest = raw.trim().strip_prefix("vox-mesh://").ok_or(TicketError::Scheme)?;
        let (before_hash, pubkey_hex) = rest.rsplit_once('#').ok_or(TicketError::Malformed)?;
        let (node_id, hostport) = before_hash.rsplit_once('@').ok_or(TicketError::Malformed)?;
        let (host, port) = hostport.rsplit_once(':').ok_or(TicketError::Malformed)?;
        let port: u16 = port.parse().map_err(|_| TicketError::Malformed)?;
        if node_id.is_empty() || host.is_empty() {
            return Err(TicketError::Malformed);
        }
        if pubkey_hex.len() != 64 || !pubkey_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(TicketError::BadKey);
        }
        let pubkey_hex = pubkey_hex.to_ascii_lowercase();
        if node_id_for_pubkey(&pubkey_hex).as_deref() != Some(node_id) {
            return Err(TicketError::IdKeyMismatch);
        }
        Ok(Self { node_id: node_id.to_string(), host: host.to_string(), port, pubkey_hex })
    }

    pub fn render(&self) -> String {
        format!("vox-mesh://{}@{}:{}#{}", self.node_id, self.host, self.port, self.pubkey_hex)
    }

    pub fn control_url(&self) -> String {
        if self.host.contains(':') && !self.host.starts_with('[') {
            format!("http://[{}]:{}", self.host, self.port)
        } else {
            format!("http://{}:{}", self.host, self.port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real key and its derived id, so a wrong derivation cannot round-trip
    /// self-consistently.
    fn valid() -> (String, String) {
        let key = "a".repeat(64);
        (node_id_for_pubkey(&key).unwrap(), key)
    }

    #[test]
    fn round_trips() {
        let (id, key) = valid();
        let t = Ticket { node_id: id, host: "192.168.1.5".into(), port: 9847, pubkey_hex: key };
        assert_eq!(Ticket::parse(&t.render()).unwrap(), t);
    }

    #[test]
    fn rejects_an_id_that_does_not_match_the_key() {
        let (_, key) = valid();
        assert_eq!(
            Ticket::parse(&format!("vox-mesh://deadbeefdeadbeef@1.2.3.4:9847#{key}")),
            Err(TicketError::IdKeyMismatch)
        );
    }

    #[test]
    fn rejects_a_short_or_non_hex_key() {
        let (id, _) = valid();
        assert_eq!(Ticket::parse(&format!("vox-mesh://{id}@h:1#abcd")), Err(TicketError::BadKey));
        assert_eq!(
            Ticket::parse(&format!("vox-mesh://{id}@h:1#{}", "z".repeat(64))),
            Err(TicketError::BadKey)
        );
    }

    #[test]
    fn rejects_other_schemes_and_junk() {
        assert_eq!(Ticket::parse("https://example.com"), Err(TicketError::Scheme));
        assert_eq!(Ticket::parse(""), Err(TicketError::Scheme));
    }

    #[test]
    fn is_case_insensitive_about_the_key() {
        let (id, key) = valid();
        let upper = key.to_ascii_uppercase();
        assert_eq!(
            Ticket::parse(&format!("vox-mesh://{id}@h:1#{upper}")).unwrap().pubkey_hex,
            key
        );
    }

    #[test]
    fn brackets_an_ipv6_host_in_the_control_url() {
        let (id, key) = valid();
        let t = Ticket { node_id: id, host: "fd00::1".into(), port: 9847, pubkey_hex: key };
        assert_eq!(t.control_url(), "http://[fd00::1]:9847");
    }
}
```

- [ ] **Step 3: Declare the modules**

`discovery/mod.rs`: `pub mod ticket; pub use ticket::{Ticket, TicketError};`
`lib.rs`, after `pub mod quota;`: `#[cfg(feature = "discovery")] pub mod discovery;`

- [ ] **Step 4: Run to verify PASS**

`cargo test -p vox-populi --features discovery discovery::ticket` — 6 tests.
**Confirm `node_id_for_pubkey` matches `vox auth trust`'s derivation** at `crates/vox-cli/src/commands/auth.rs:126`. A mismatch silently produces rows nothing can look up.

- [ ] **Step 5: Commit**

### Task 1.2: `vox mesh ticket` and `vox auth trust <ticket>`

**Files:** Modify `crates/vox-cli/src/commands/auth.rs`; add the ticket command.

- [ ] **Step 1: Write the failing test**

```rust
/// Accept either a raw 64-hex pubkey (existing behaviour) or a full ticket.
pub(crate) enum TrustInput {
    Pubkey(String),
    Ticket(Box<vox_populi::discovery::Ticket>),
}

pub(crate) fn parse_trust_input(raw: &str) -> anyhow::Result<TrustInput> { /* … */ }

#[cfg(test)]
mod trust_input_tests {
    use super::*;
    use vox_populi::discovery::ticket::node_id_for_pubkey;

    #[test]
    fn accepts_a_bare_pubkey_as_before() {
        assert!(matches!(parse_trust_input(&"a".repeat(64)).unwrap(), TrustInput::Pubkey(_)));
    }

    #[test]
    fn accepts_a_ticket() {
        let key = "a".repeat(64);
        let id = node_id_for_pubkey(&key).unwrap();
        let t = format!("vox-mesh://{id}@192.168.1.5:9847#{key}");
        assert!(matches!(parse_trust_input(&t).unwrap(), TrustInput::Ticket(_)));
    }

    #[test]
    fn a_tampered_ticket_is_rejected_with_a_useful_message() {
        let key = "a".repeat(64);
        let e = parse_trust_input(&format!("vox-mesh://wrongid@h:1#{key}")).unwrap_err();
        assert!(e.to_string().contains("does not match"));
    }
}
```

- [ ] **Step 2: Run to verify it fails, then implement and pass**

`cargo test -p vox-cli trust_input_tests`

- [ ] **Step 3: Print this node's ticket**

Add `vox mesh ticket [--json] [--host <h>]`, rendering from the local identity's pubkey, `detect_lan_ip()` (Task 2.1) or `--host`, and the configured port. With no routable address, print loopback and say so — still correct for a same-machine test.

- [ ] **Step 4: Regenerate the CLI surface SSOT**

```bash
cargo run -p vox-cli -- ci command-sync
```

A **new subcommand does** drift `cli-command-surface.generated.md`, unlike Task 3.1's deprecation.

- [ ] **Step 5: Commit**

### Task 1.3: End-to-end trust over a ticket

**Files:** Create `crates/vox-populi/tests/ticket_pairing.rs`

- [ ] **Step 1: Write the integration test**

```rust
#[tokio::test]
async fn a_ticket_pairs_two_nodes_and_an_untrusted_peer_is_refused() {
    let (a, b) = two_nodes().await;
    // Before pairing, B cannot dispatch to A.
    assert_eq!(
        b.dispatch_to(&a, "probe").await.unwrap_err().status(),
        StatusCode::UNAUTHORIZED
    );
    a.trust(Ticket::parse(&b.ticket()).unwrap());
    b.trust(Ticket::parse(&a.ticket()).unwrap());
    assert!(b.dispatch_to(&a, "probe").await.is_ok());
}
```

This is the test the whole design exists to make pass. It must fail before Task 0.2 and pass after this task. Replaces `crates/vox-populi/tests/pairing_e2e.rs`, currently a one-line placeholder comment.

- [ ] **Step 2: Commit**

---

## Phase 2 — mDNS LAN discovery (T2), the convenience layer

### Task 2.1: Bind resolution

**Files:** Create `crates/vox-populi/src/discovery/bind.rs`

Created **before** the mDNS module that calls into it. (Revision 1 had this backwards and was non-executable.)

- [ ] **Step 1: Write the failing test**

```rust
//! Tier-aware bind resolution. `0.0.0.0` is never synthesized.

use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindTier { Loopback, Lan, Overlay(IpAddr) }

/// Whether an address is safe to bind and advertise on the LAN tier.
///
/// `detect_lan_ip` is a *default-route* probe, not a LAN probe: on a
/// directly-attached host it returns the PUBLIC address. Binding that by
/// default would publish the control plane to the internet.
pub fn is_advertisable(ip: IpAddr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private() || v4.is_link_local() || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(v6) => {
            let s = v6.segments()[0];
            (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
    }
}

// vox:defactored-from vox-cli-share 2026-09-04
pub fn detect_lan_ip() -> Option<IpAddr> {
    let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?; // no packet is sent; air-gap safe
    let ip = sock.local_addr().ok()?.ip();
    is_advertisable(ip).then_some(ip)
}

pub fn resolve_bind(tier: BindTier, explicit: Option<&str>, port: u16) -> String {
    if let Some(e) = explicit {
        let e = e.trim();
        if !e.is_empty() {
            return e.to_string();
        }
    }
    let ip = match tier {
        BindTier::Loopback => IpAddr::V4(Ipv4Addr::LOCALHOST),
        BindTier::Overlay(ip) => ip,
        BindTier::Lan => detect_lan_ip().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
    };
    match ip {
        IpAddr::V6(v6) => format!("[{v6}]:{port}"),
        IpAddr::V4(v4) => format!("{v4}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_addresses_are_never_advertisable() {
        // The bug this predicate exists to prevent: a VPS binding its public IP.
        for public in ["8.8.8.8", "203.0.113.7", "2606:4700::1111"] {
            assert!(!is_advertisable(public.parse().unwrap()), "{public} must not be advertised");
        }
    }

    #[test]
    fn private_link_local_and_cgnat_are_advertisable() {
        for ok in ["192.168.1.5", "10.0.0.5", "172.16.0.1", "169.254.1.1", "100.107.222.96", "fd00::1"] {
            assert!(is_advertisable(ok.parse().unwrap()), "{ok} must be advertisable");
        }
    }

    #[test]
    fn useless_addresses_are_rejected() {
        for bad in ["127.0.0.1", "0.0.0.0", "::1", "::", "224.0.0.251"] {
            assert!(!is_advertisable(bad.parse().unwrap()), "{bad}");
        }
    }

    #[test]
    fn loopback_tier_binds_loopback() {
        assert_eq!(resolve_bind(BindTier::Loopback, None, 9847), "127.0.0.1:9847");
    }

    #[test]
    fn overlay_tier_binds_the_overlay_address() {
        let ip: IpAddr = "100.107.222.96".parse().unwrap();
        assert_eq!(resolve_bind(BindTier::Overlay(ip), None, 9847), "100.107.222.96:9847");
    }

    #[test]
    fn explicit_bind_wins_and_blank_falls_through() {
        let ip: IpAddr = "100.64.0.1".parse().unwrap();
        assert_eq!(resolve_bind(BindTier::Overlay(ip), Some("192.168.1.9:1234"), 9847), "192.168.1.9:1234");
        assert_eq!(resolve_bind(BindTier::Loopback, Some("  "), 9847), "127.0.0.1:9847");
    }

    #[test]
    fn ipv6_is_bracketed() {
        let ip: IpAddr = "fd7a:115c:a1e0::1".parse().unwrap();
        assert_eq!(resolve_bind(BindTier::Overlay(ip), None, 9847), "[fd7a:115c:a1e0::1]:9847");
    }

    #[test]
    fn no_tier_ever_synthesizes_a_wildcard() {
        for tier in [BindTier::Loopback, BindTier::Lan, BindTier::Overlay("10.0.0.5".parse().unwrap())] {
            let b = resolve_bind(tier, None, 9847);
            assert!(!b.starts_with("0.0.0.0") && !b.starts_with("[::]"), "{tier:?} -> {b}");
        }
    }
}
```

- [ ] **Step 2: Declare `pub mod bind;`, run, verify PASS (8 tests)**

`cargo test -p vox-populi --features discovery discovery::bind`

- [ ] **Step 3: Commit**

### Task 2.2: mDNS announce and browse

**Files:** Create `crates/vox-populi/src/discovery/mdns.rs`

**Produces:** `DiscoveredPeer { node_id, pubkey_hex, fingerprint, addr, port, last_seen_unix_ms, trusted }`, `NodeAnnouncement`, `DiscoveryHandle::{start, browse_only, peers, shutdown}`, `PeerMap`, `scope_matches`, `pick_addr`.

Seven corrections, each a real defect in revision 1:

1. **No staleness clock.** `ServiceResolved` fires **once per instance**, not per announcement, so a self-managed 90s window empties the list permanently. `mdns-sd` owns liveness and emits `ServiceRemoved` on TTL expiry and goodbye. Online-vs-offline comes from the control-plane health check.
2. **Key the map by DNS-SD fullname**, not the announced `node_id`, so an impostor cannot silently replace a real peer.
3. **Announce the pubkey**, not only a fingerprint — trust must bind to key material.
4. **`shutdown(&self)`**, not `self`: the production handle lives in a `OnceLock` and cannot be moved out.
5. **Select the address by family**, rejecting link-local IPv6 (no scope id available) and loopback.
6. **Do not announce without a routable address** — publishing `127.0.0.1` gives every peer an entry that health-checks green against its own loopback.
7. **Cap the map at 256** with oldest-eviction; the key is attacker-chosen.

- [ ] **Step 1: Pin the crate and confirm the API**

```toml
mdns-sd = { version = "=0.20", optional = true } # pinned: 20 breaking releases in 78; 0.15 changed ServiceResolved's payload
```

Confirm against the pinned docs: `ServiceInfo::new` (six args), `get_property_val_str() -> Option<&str>`, `get_addresses() -> &HashSet<IpAddr>`, `ServiceEvent` shapes, `unregister`/`shutdown` on `&self`. **0.20's `ServiceResolved` carries `ResolvedService`, not `ServiceInfo`** — adapt the browse arm to the real type rather than downgrading.

- [ ] **Step 2: Write the failing tests (pure logic only — no network in CI)**

```rust
/// Whether a peer's scope matches ours.
///
/// Fail *closed*: a scoped node must not accept a peer that omits its scope,
/// or an attacker bypasses the filter by simply not setting the TXT key.
pub(crate) fn scope_matches(ours: Option<&str>, theirs: Option<&str>) -> bool {
    match ours { Some(a) => theirs == Some(a), None => true }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scoped_node_rejects_a_peer_that_omits_its_scope() {
        assert!(!scope_matches(Some("teamA"), None), "omitting scope must not bypass the filter");
    }

    #[test]
    fn a_scoped_node_rejects_a_different_scope() {
        assert!(!scope_matches(Some("teamA"), Some("teamB")));
    }

    #[test]
    fn an_unscoped_node_lists_everyone() {
        assert!(scope_matches(None, Some("teamA")));
        assert!(scope_matches(None, None));
    }

    #[test]
    fn two_peers_claiming_one_node_id_are_both_listed() {
        let mut m = PeerMap::default();
        m.observe("inst-a._vox-mesh._tcp.local.", peer("n1", "aaaa", "192.168.1.5"));
        m.observe("inst-b._vox-mesh._tcp.local.", peer("n1", "bbbb", "192.168.1.99"));
        assert_eq!(m.list().len(), 2, "an impostor must not replace the real peer");
    }

    #[test]
    fn the_peer_map_is_capped_against_a_flooding_neighbour() {
        let mut m = PeerMap::default();
        for i in 0..1000 {
            m.observe(&format!("i{i}._vox-mesh._tcp.local."), peer(&format!("n{i}"), "aa", "192.168.1.5"));
        }
        assert!(m.list().len() <= 256);
    }

    #[test]
    fn a_routable_ipv4_is_preferred_over_link_local_ipv6() {
        let addrs = ["fe80::1".parse().unwrap(), "192.168.1.5".parse().unwrap()].into_iter().collect();
        assert_eq!(pick_addr(&addrs).unwrap().to_string(), "192.168.1.5");
    }

    #[test]
    fn link_local_ipv6_alone_yields_no_address() {
        // No scope id is available, so http://[fe80::1] is unroutable.
        let addrs = ["fe80::1".parse().unwrap()].into_iter().collect();
        assert!(pick_addr(&addrs).is_none());
    }
}
```

- [ ] **Step 3: Run to verify they fail, implement, verify PASS (7 tests)**

- [ ] **Step 4: Add the round-trip test, ignored by default**

`#[ignore]` test that registers a service and browses for it, asserting the TXT pubkey survives. Run it in Phase 5, not CI — multicast in CI containers is unreliable.

- [ ] **Step 5: Commit**

### Task 2.3: Trust resolution, batched and pubkey-bound

**Files:** Modify `crates/vox-populi/src/discovery/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
/// Stamp `trusted` from the local registry. Binds to the **pubkey**, with a
/// node_id cross-check: a peer announcing a trusted id under a different key
/// is not trusted.
///
/// One registry read per batch — `is_trusted` re-reads and re-parses the whole
/// file per call (vox-identity/src/trust.rs:128), and three GUI callers poll
/// this every few seconds.
pub fn resolve_trust(peers: Vec<DiscoveredPeer>, reg: &TrustedNodeRegistry) -> Vec<DiscoveredPeer> {
    let rows = reg.list().unwrap_or_default(); // fail closed
    peers.into_iter().map(|mut p| {
        p.trusted = !p.pubkey_hex.is_empty()
            && rows.iter().any(|n| n.pubkey_hex == p.pubkey_hex && n.node_id == p.node_id);
        p
    }).collect()
}

#[cfg(test)]
mod trust_tests {
    use super::*;

    fn reg_with(node_id: &str, key: &str) -> (tempfile::TempDir, TrustedNodeRegistry) {
        let dir = tempfile::tempdir().unwrap();
        let reg = TrustedNodeRegistry::at(dir.path().join("t.json"));
        reg.add(node_id.into(), key.into(), None).unwrap();
        (dir, reg)
    }

    #[test]
    fn an_impostor_reusing_a_trusted_node_id_is_not_trusted() {
        let (_d, reg) = reg_with("n1", "aaaa");
        let mut imposter = peer("n1", "192.168.1.99");
        imposter.pubkey_hex = "bbbb".into();
        assert!(!resolve_trust(vec![imposter], &reg)[0].trusted);
    }

    #[test]
    fn a_matching_key_and_id_is_trusted() {
        let (_d, reg) = reg_with("n1", "aaaa");
        let mut p = peer("n1", "192.168.1.5");
        p.pubkey_hex = "aaaa".into();
        assert!(resolve_trust(vec![p], &reg)[0].trusted);
    }

    #[test]
    fn an_empty_pubkey_never_matches_even_an_empty_stored_row() {
        let (_d, reg) = reg_with("n1", "");
        let p = peer("n1", "192.168.1.5"); // pubkey_hex defaults empty
        assert!(!resolve_trust(vec![p], &reg)[0].trusted);
    }

    #[test]
    fn trust_survives_an_address_change() {
        let (_d, reg) = reg_with("n1", "aaaa");
        let mut moved = peer("n1", "192.168.1.77");
        moved.pubkey_hex = "aaaa".into();
        assert!(resolve_trust(vec![moved], &reg)[0].trusted, "trust binds to the key, not the address");
    }
}
```

- [ ] **Step 2: Run to verify fail, then pass. Commit.**

### Task 2.4: Process-correct discovery access

**Files:** Modify `crates/vox-populi/src/discovery/mod.rs`

Revision 1 stored one global and read it from a different process. There are three; a `OnceLock` spans none.

- [ ] **Step 1: Write the failing test**

```rust
use std::sync::OnceLock;

static GLOBAL: OnceLock<Option<DiscoveryHandle>> = OnceLock::new();

/// Announce **and** browse. Called by `vox populi serve` — the process that is
/// itself a mesh member. Never fails: discovery is an enhancement.
pub fn start_global(announce: NodeAnnouncement) {
    GLOBAL.get_or_init(|| DiscoveryHandle::start(announce).map_err(log_warn).ok());
}

/// Browse only — no announcement, no node identity. Safe from any process that
/// only wants to *see* peers, such as the orchestrator daemon serving
/// `vox_mesh_nodes`. Lazy: the first read starts it.
///
/// ponytail: lazy-start on first read; add an explicit start hook if the
/// one-interval mDNS warm-up on the first poll becomes user-visible.
pub fn global_peers_browsing() -> Vec<DiscoveredPeer> {
    peers_of(GLOBAL.get_or_init(|| DiscoveryHandle::browse_only().map_err(log_warn).ok()).as_ref())
}

/// Peers with trust resolved. Lives here so callers need no `vox-identity` edge.
pub fn global_peers_with_trust() -> Vec<DiscoveredPeer> {
    resolve_trust(global_peers_browsing(), &TrustedNodeRegistry::new())
}

/// Extracted so the never-fatal contract is testable without a network.
pub(crate) fn peers_of(h: Option<&DiscoveryHandle>) -> Vec<DiscoveredPeer> {
    h.map(|h| h.peers()).unwrap_or_default()
}

#[cfg(test)]
mod global_tests {
    use super::*;

    #[test]
    fn a_failed_start_yields_an_empty_list_not_a_panic() {
        assert!(peers_of(None).is_empty());
    }
}
```

No test calls `start_global`: a `OnceLock` cannot be reset, so such a test would be order-dependent inside the crate's single test binary. The pure `peers_of` carries the contract.

- [ ] **Step 2: Run to verify fail, then pass. Commit.**

### Task 2.5: Tier-aware bind in `populi up`

**Files:** Modify `crates/vox-ml-cli/src/commands/{populi_lifecycle.rs,populi_lifecycle_cmd.rs}`

- [ ] **Step 1: Make `--bind` optional**

Change `bind: String` with `default_value` to `bind: Option<String>`. String-comparing against the default silently overrides a user who deliberately types it.

- [ ] **Step 2: Write the failing test against production functions**

Revision 1 defined the tier logic *inside its own test module*, so reverting the implementation left all three tests green. Test real functions:

```rust
/// Choose the bind tier. Pure — the overlay probe is injected.
pub(crate) fn bind_tier_for(mode: PopuliConnectivityMode, overlay_ip: Option<IpAddr>) -> BindTier {
    match (mode, overlay_ip) {
        (PopuliConnectivityMode::Overlay, Some(ip)) => BindTier::Overlay(ip),
        (PopuliConnectivityMode::Overlay, None) => BindTier::Loopback,
        (PopuliConnectivityMode::Lan, _) => BindTier::Lan,
    }
}

#[cfg(test)]
mod bind_tests {
    use super::*;

    #[test]
    fn overlay_mode_binds_the_overlay_address() {
        let ip: IpAddr = "100.107.222.96".parse().unwrap();
        assert_eq!(bind_tier_for(PopuliConnectivityMode::Overlay, Some(ip)), BindTier::Overlay(ip));
    }

    #[test]
    fn overlay_without_an_ip_stays_on_loopback() {
        assert_eq!(bind_tier_for(PopuliConnectivityMode::Overlay, None), BindTier::Loopback);
    }

    #[test]
    fn lan_mode_ignores_an_overlay_address() {
        let ip: IpAddr = "100.64.0.1".parse().unwrap();
        assert_eq!(bind_tier_for(PopuliConnectivityMode::Lan, Some(ip)), BindTier::Lan);
    }

    #[test]
    fn the_spawned_child_receives_the_resolved_bind_not_the_raw_flag() {
        // The actual regression: `up` computed a control URL from one value and
        // passed a different one to the child.
        assert!(serve_child_args("192.168.1.9:9847").contains(&"192.168.1.9:9847".to_string()));
    }
}
```

- [ ] **Step 3: Run with `--features populi`, not default**

`cargo test -p vox-ml-cli --features populi bind_tests` — `populi_lifecycle.rs` is `#[cfg(feature = "populi")]`; a default run compiles none of it and reports a **false green**.

- [ ] **Step 4: Apply to the real path**

Compute `effective_bind` once, derive `control_url` from it, pass it via `serve_child_args`, store it in `PopuliDaemonState.bind`, and confirm Task 0.3's guard reads the same value.

- [ ] **Step 5: Commit**

### Task 2.6: Overlay probing extracted

**Files:** Create `crates/vox-ml-cli/src/commands/populi_overlay.rs`; modify `populi_lifecycle.rs`

**Scope correction:** revision 1 claimed the exit-code probe conflates "installed" with "connected" for Tailscale. **That was false** — `overlay_diag_tailscale` already separates them. The real conflation is in `overlay_diag_wireguard` (`let connected = available;`) and `overlay_diag_tunnel`. This task fixes the Windows `PATH` issue and the probe cost, and documents honestly that overlay+WireGuard stays non-functional (there is no WireGuard arm in `overlay_control_url`).

- [ ] **Step 1: Write the failing test**

```rust
/// Candidate paths in probe order. Split out so the Windows path is asserted
/// without installing Tailscale.
pub fn tailscale_candidates() -> &'static [&'static str] {
    &["tailscale",
      r"C:\Program Files\Tailscale\tailscale.exe",
      "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
      "/usr/bin/tailscale", "/usr/local/bin/tailscale", "/opt/homebrew/bin/tailscale"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_path_lookup_is_tried_first() {
        assert_eq!(tailscale_candidates()[0], "tailscale");
    }

    #[test]
    fn the_windows_install_location_is_probed() {
        assert!(tailscale_candidates().contains(&r"C:\Program Files\Tailscale\tailscale.exe"),
            "the Windows installer does not modify PATH");
    }

    // The five `parse_tailnet_status` tests carry over from revision 1 unchanged
    // — the audit verified all of them, including the `Self: null` short-circuit.
}
```

- [ ] **Step 2: Collapse to one probe, memoized, with a timeout**

Drop the separate `version` probe (if `status --json` runs, the binary exists) and memoize `Option<TailnetStatus>` in a `OnceLock`. Revision 1 spawned up to twelve subprocesses per status line with no timeout on any — a wedged `tailscaled` hangs `vox populi status` forever.

- [ ] **Step 3: Verify PASS, delete the superseded functions, commit**

---

## Phase 3 — SSOT cleanup

### Task 3.1: Retire the duplicate peer-trust store

**Files:** Modify `crates/vox-ml-cli/src/commands/populi_join.rs`; amend the mesh SSOT

Revision 1 planned to *migrate* `config.toml` peers into the trust registry. **Dropped** — a manifest URL is not evidence of a key, and migrating would create exactly the empty-pubkey rows Phase 0 forbids. The key is written by `vox populi join` and read by nothing, so it is removed outright.

- [ ] **Step 1: Confirm nothing reads it**

```bash
rg "mesh\.federation_peers" crates/
```

Expected: only the writer. If a reader appears, stop and reassess.

- [ ] **Step 2: Deprecate the command**

```rust
// vox-deprecated-since="0.6.0" retire-by="0.7.0" reason="ssot-peer-trust" canonical="vox auth trust"
```

Print: `vox populi join is deprecated. Pair with a ticket instead: vox auth trust <vox-mesh://...>. Get one from the other machine with: vox mesh ticket`. Stop writing the config key.

- [ ] **Step 3: File the SSOT amendment**

`mesh-and-language-distribution-ssot-2026.md` line 101 states a binding non-goal: *"Paired peers + GitHub attestation are the binary gates."* Add a dated amendment recording that LAN pairing by ticket supersedes attestation for local peers, that P6-T7's invite flow is deliberately replaced, and that the attestation subsystem (`fetch_and_verify`, `device_flow`, `PublicAttestationManifest`) has no non-test callers. **Do not silently contradict a ratified SSOT.**

- [ ] **Step 4: Commit**

### Task 3.2: Remove the crate-wide dead-code allow

**Files:** Modify `crates/vox-plugin-populi-mesh/src/lib.rs:11-13`

- [ ] **Step 1: Remove it, then inventory what it hid**

```bash
cargo clippy -p vox-plugin-populi-mesh --all-targets 2>&1 | rg "never used|never read" | sort -u
```

Expect substantially more than revision 1 anticipated — spec §2 documents ~2,500 unreferenced lines across pairing, attestation, and federation.

- [ ] **Step 2: Resolve each — wire, delete, or per-item allow naming the FFI entry point.** Prefer deletion. Anything Phase 0 wires (`verify_against_trust`) is no longer dead.

- [ ] **Step 3: Verify clean, tests pass, commit**

---

## Phase 4 — GUI

### Task 4.1: Serve pending peers and discovery state

**Files:** Modify `crates/vox-orchestrator-mcp/src/{populi_tools.rs,Cargo.toml}`; `crates/vox-gui/Cargo.toml`; `crates/vox-orchestrator-d/Cargo.toml`

- [ ] **Step 1: Enable the right feature in the right crates**

Add `mesh-discovery = ["vox-populi/discovery-mdns"]` to `vox-orchestrator-mcp`, and enable it from **both** `vox-gui` and `vox-orchestrator-d`. Do **not** attach it to `populi-transport` — that gates HTTP machinery and is off in every shipped binary, which is why revision 1's field would have been permanently empty.

- [ ] **Step 2: Write the failing test**

```rust
pub(crate) fn pending_peers_json(peers: &[vox_populi::discovery::DiscoveredPeer]) -> Vec<Value> {
    peers.iter().filter(|p| !p.trusted).map(|p| json!({
        "node_id": p.node_id,
        "pubkey_hex": p.pubkey_hex,
        "fingerprint": p.fingerprint,
        "addr": p.addr.to_string(),
        "port": p.port,
        "control_url": p.control_url(),
        "last_seen_unix_ms": p.last_seen_unix_ms,
    })).collect()
}

#[cfg(test)]
mod pending_tests {
    use super::*;

    #[test]
    fn the_pubkey_is_carried_so_trust_can_bind_to_it() {
        let out = pending_peers_json(&[peer("n1", false)]);
        assert!(!out[0]["pubkey_hex"].as_str().unwrap().is_empty(),
            "trusting without a key produces an unusable, spoofable row");
    }

    #[test]
    fn trusted_peers_are_omitted() {
        assert!(pending_peers_json(&[peer("n1", true)]).is_empty());
    }

    #[test]
    fn fingerprint_is_carried_for_human_verification() {
        assert_eq!(pending_peers_json(&[peer("n1", false)])[0]["fingerprint"], "ab:cd");
    }
}
```

- [ ] **Step 3: Wire without a `vox-identity` edge**

```rust
let pending = pending_peers_json(&vox_populi::discovery::global_peers_with_trust());
```

**No `use vox_identity::...` in this crate** — that edge does not exist and must not be added.

- [ ] **Step 4: Emit `discovery_state`**

Add `"discovery_state": "disabled" | "failed" | "running"`, and `"control_plane": "unconfigured"` when no URL is set, so the GUI can distinguish "firewall blocked" from "nobody home". The spec requires an empty mesh to say why.

- [ ] **Step 5: Add the integration test that would have caught revision 1**

Assert `mesh_nodes` returns `pending_peers` sourced from live discovery. Revision 1's three unit tests on `pending_peers_json` passed under all three of its wiring failures.

- [ ] **Step 6: Commit**

### Task 4.2: MeshView — ticket paste, pending peers, local ticket

**Files:** Modify `crates/vox-gui/ui/src/hooks/useMeshNodes.ts`, `components/surfaces/Mesh/MeshView.tsx`, `MeshView.test.tsx`

- [ ] **Step 1: Write the failing tests against the real harness**

There is **no** `renderMeshView` helper. The suite mocks `@tauri-apps/api/core`'s `invoke` (which `voxTransport.invokeMcpTool` bottoms out in) via a module-scope `invokeMock`, and must answer **two** tools. `pushToast` is a required prop. Add below the existing mock:

```tsx
function mockMeshNodes(result: Record<string, unknown>) {
  invokeMock.mockImplementation((cmd: string, args?: any) => {
    if (cmd === 'invoke_mcp_tool' && args?.tool === 'vox_mesh_nodes') {
      return Promise.resolve({ tool: 'vox_mesh_nodes', is_error: false, result });
    }
    if (cmd === 'invoke_mcp_tool' && args?.tool === 'vox_mesh_queue_stats') {
      return Promise.resolve({ tool: 'vox_mesh_queue_stats', is_error: false, result: {} });
    }
    return Promise.resolve(null);
  });
}
```

```tsx
describe('MeshView pending peers', () => {
  const pending = [{
    node_id: 'peer-b', fingerprint: 'ab:cd:ef:12', pubkey_hex: 'aa'.repeat(32),
    addr: '192.168.1.42', port: 9847, control_url: 'http://192.168.1.42:9847',
    last_seen_unix_ms: Date.now(),
  }];

  it('lists a discovered untrusted peer with its fingerprint', async () => {
    mockMeshNodes({ source: 'local_registry', nodes: [], pending_peers: pending });
    render(<MeshView pushToast={vi.fn()} />);
    expect(await screen.findByText('peer-b')).toBeInTheDocument();
    expect(screen.getByText(/ab:cd:ef:12/)).toBeInTheDocument();
  });

  it('trusts with the peer pubkey, never an empty string', async () => {
    mockMeshNodes({ source: 'local_registry', nodes: [], pending_peers: pending });
    render(<MeshView pushToast={vi.fn()} />);
    (await screen.findByRole('button', { name: /^trust/i })).click();
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      'trust_mesh_node',
      expect.objectContaining({ nodeId: 'peer-b', pubkeyHex: 'aa'.repeat(32) }),
    ));
  });

  it('distinguishes a blocked network from an empty one', async () => {
    mockMeshNodes({ source: 'local_registry', nodes: [], pending_peers: [], discovery_state: 'failed' });
    render(<MeshView pushToast={vi.fn()} />);
    expect(await screen.findByText(/discovery unavailable/i)).toBeInTheDocument();
  });
});
```

Anchor the button matcher (`/^trust/i`) — it will otherwise collide with "Untrust" from Step 5. The existing suite asserts every button has `type="button"`.

- [ ] **Step 2: Run to verify they fail**

`cd crates/vox-gui/ui && pnpm vitest run MeshView`

- [ ] **Step 3: Extend `NodesResult`**

Add `pending_peers?: PendingPeer[]` and `discovery_state?: 'disabled' | 'failed' | 'running'`; coerce `pending_peers` to `[]` in `fetchMeshNodesResult` so consumers never see `undefined`. `PendingPeer` **includes `pubkey_hex: string`**.

- [ ] **Step 4: Render, with the real Toast shape**

The `Toast` type is `{ tone: 'ok'|'warn'|'info'; title: string; body?: string; cause: ToastCause }` — there is no `kind` or `message`. Getting this wrong fails `vox ci gui-honesty`, a required job that exists to catch exactly this.

```tsx
const trustPeer = useCallback(async (p: PendingPeer) => {
  try {
    await invoke<boolean>('trust_mesh_node', {
      nodeId: p.node_id, pubkeyHex: p.pubkey_hex, label: p.control_url,
    });
    pushToast({ tone: 'ok', title: 'Peer trusted', body: p.node_id, cause: 'backend-ok' });
    await refresh();
  } catch (err) {
    pushToast({ tone: 'warn', title: 'Trust failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
  }
}, [pushToast, refresh]);
```

Declare it **after** `refresh` (~line 104) or it is a TDZ error. Tauri args are camelCase — settled against `SettingsView.tsx:192` calling this same command; do not hedge.

The pending section is a sibling `Glass` with **`col-span-12`** (the root is a 12-column grid; an unspanned `<section>` renders as a squashed 1/12 strip). Fold the empty-state copy into the **existing** empty state rather than adding a second competing one.

Use design tokens, not raw Tailwind colors. `docs/agents/gui-honesty-findings/Mesh.json` already flags `statusTone()` (`MeshView.tsx:61`) at `severity: med` for a hardcoded palette — do not copy that pattern into the new rows. Adding a Trust control also adds a `behavioral` entry to that findings file; regenerate it rather than letting it drift.

- [ ] **Step 5: Add the ticket UI — three things revision 1 omitted**

- **Paste box** accepting a `vox-mesh://` ticket → `invoke('trust_mesh_ticket', { ticket })`. This is the universal path and must be at least as prominent as the pending list.
- **This node's own ticket** with a copy button — the other machine needs it.
- **Untrust** action, so a mistaken trust is reversible without hunting through Settings.

- [ ] **Step 6: Render trusted peers no control plane reports**

On a LAN-only two-node setup there is no control plane, so a peer filtered out of `pending_peers` on trust would **vanish from the UI entirely**. Merge trusted peers into the table with a trust column, or give them their own section.

- [ ] **Step 7: Show the local fingerprint**

Otherwise the comparison ceremony is unperformable: it lives only in Settings and reads `(locked — provide master password to view)` on a fresh install. Render it in the pending-section header with an unlock affordance.

- [ ] **Step 8: Run tests and the honesty gate**

```bash
cd crates/vox-gui/ui && pnpm test && pnpm typecheck
cargo run -p vox-cli -- ci gui-honesty
```

- [ ] **Step 9: Commit**

### Task 4.3: MeshWidget counts

**Files:** Modify `crates/vox-gui/ui/src/components/dashboard/widgets/MeshWidget.tsx`; create `MeshWidget.test.tsx`

Revision 1 said to "derive from the same `useMeshNodesFull` result the widget already consumes." **It consumes no such thing** — it is 14 lines taking `{ data }: { data: DashboardData }` and reading `data.peers`, with no hooks and no MCP call. Adding a pending count is a real change to the component, not a derivation.

- [ ] **Step 1: Write the failing test**

Assert the combined string once (`/1 online · 1 pending/`); two matchers against one text node is a false pattern. Mock `@tauri-apps/api/core` — `Dashboard.test.tsx` does not, and this widget will now poll.

- [ ] **Step 2: Implement**

Keep the literal label `Mesh Peers` (`Dashboard.test.tsx:355` asserts on it). The tile is a 4-of-12 slot; the count must **replace** the existing third line, not add a fourth. Poll at 30s, not MeshView's 5s.

- [ ] **Step 3: Run the full frontend suite, commit**

---

## Phase 5 — Two-node verification

### Task 5.1: Verify and document

**Prerequisites:** two machines on one LAN. Run `vox run scripts/gui-build.vox` once per worktree, or `cargo build -p vox-gui` fails inside `tauri-build`.

- [ ] **Step 1: Ticket path first (the universal one)**

On B: `vox mesh ticket`. On A: paste into the GUI. Reciprocate. Expected: mutual trust with no discovery involved. **This must work before mDNS is tested** — it is the path that has to work everywhere.

- [ ] **Step 2: Refusal before trust**

```bash
vox populi dispatch ./probe.vox --node <B-node-id>
```

Note the real signature: `script` is a **positional `PathBuf`**; there is no `--source`. Expected: refused, naming trust. A success means Phase 0 did not land — blocking.

- [ ] **Step 3: Dispatch after trust**

Same command. Expected: executes on B and returns. Confirm the lease on both via `curl $CONTROL_URL/v1/populi/exec/leases` — `vox populi exec-leases` is **not** a command.

- [ ] **Step 4: mDNS path**

`vox populi up --mode lan` on both. Expected within ~60s: each lists the other as pending. If empty, check UDP 5353 — on Windows the first run raises a firewall prompt that must be accepted for private networks, and a **non-admin user cannot accept it**. That is an expected outcome, not a bug: fall back to the ticket and record which network class you were on.

- [ ] **Step 5: GUI ceremony (spec Requirement 6)**

Pending peer visible with fingerprint → compare against B's fingerprint **on the Mesh surface** (not `vox populi identity show`, which prints the *federation signing key* in a different format from a different store — they can never match) → Trust → persists to `~/.vox/trusted_nodes.json` with a **non-empty** `pubkey_hex` → peer stays visible as trusted → dispatch from the GUI runs on B.

**Capture a screenshot.** This is the deliverable for Requirement 6.

- [ ] **Step 6: Departure**

Stop vox on B. A reflects it via the control-plane health check. `ServiceRemoved` fires on goodbye or TTL expiry; do not assert a specific window.

- [ ] **Step 7: The no-internet requirement**

Disconnect both from the internet, keep them on the LAN, repeat Steps 1 and 3. Expected: unchanged. This is the requirement distinguishing this design from the rejected one — a failure here is blocking.

- [ ] **Step 8: Write the quickstart**

`docs/src/how-to/populi-two-node-quickstart.md`, frontmatter `category: "How-To Guides"` (verified against the canonical vocabulary). **Lead with the ticket**, present mDNS as the shortcut, and state plainly which networks it will not work on.

- [ ] **Step 9: Full gate**

```bash
vox ci pre-push --complete
```

---

## Appendix — verified correct, do not relitigate

The audit confirmed these against the codebase. Treat as settled:

- **`mdns-sd` API shapes** — `ServiceInfo::new` takes six args; `HashMap<String,String>` implements `IntoTxtProperties`; `get_property_val_str` returns `Option<&str>`; `unregister`/`shutdown` take `&self` and return `Receiver`s; `ServiceDaemon` is `Clone + Send + Sync`. Port 5353 is shared via `SO_REUSEADDR`, so Bonjour and Avahi coexist.
- **Self-filtering by `node_id` is required and sufficient** — `mdns-sd` does not exclude the daemon's own registrations from browse results.
- **Tauri args are camelCase** at the top level; nested struct fields stay snake_case.
- **`mdns-sd` is license- and policy-clean** — MIT/Apache-2.0, no crypto crates, no cmake/nasm. (`ring` and `aegis` are already in `Cargo.lock` despite the AGENTS.md ban, so that policy is aspirational, not gate-enforced.)
- **The edge ratchet models edges, not features** — adding a feature to an existing dep is free.
- **Arch-check LoC budgets are `warn`/`off`** — all three touched crates are already over budget on `main`; this plan cannot fail that gate.
- **The test-first detector is file-granular**, not per-function.
- **`category: "How-To Guides"`** is in the canonical vocabulary.
- **`vox auth trust` exists** (`crates/vox-cli/src/commands/auth.rs:16`) and takes a 64-hex pubkey.
- **`parse_tailnet_status`'s field names and the `Self: null` short-circuit** are correct; selecting IPv4 by family rather than position is right.
- **`resolve_bind`'s IPv6 bracketing, blank-explicit fallthrough, and no-wildcard behavior** are correct.
- **`detect_lan_ip`'s UDP-connect trick sends no packet** and is air-gap safe; defactoring rather than taking a `vox-cli-share` edge is correct under rule 3.
- **Default `vox populi up` (no `--insecure-local`) does generate a token** — the control plane is not open by default *today*; the risk is created only by widening the bind.
- **No `contracts/` change is required** for `pending_peers` — the crate has no MCP output schema.
