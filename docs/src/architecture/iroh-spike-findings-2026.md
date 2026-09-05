---
title: "iroh transport spike — measured findings"
description: "Answers to the six load-bearing questions from Task 0.2 of the populi-mesh iroh plan, measured between a macOS and a Windows machine on 2026-09-04."
category: "Architecture SSOTs"
status: "current"
---

# iroh transport spike — measured findings

Throwaway spike for [Task 0.2](../../superpowers/plans/2026-09-04-populi-mesh-iroh-transport.md)
of the populi-mesh iroh plan. The spike itself is deleted; this page is its
output. **Every number below was measured, not estimated.**

## Rig

| | Mac (dialer) | BLAPTOP04 (listener) |
|---|---|---|
| OS / triple | macOS 26.5, `aarch64-apple-darwin` | Windows, `x86_64-pc-windows-msvc` |
| rustc | 1.96.0 (workspace pin) / 1.98.0 for the spike | 1.98.0 |
| LAN | 192.168.50.208 | 192.168.50.83 |
| Tailnet | — | 100.107.222.96 |
| Repo commit | `f48dbc810` | `f48dbc810` |

Resolved crate versions: `iroh 1.1.0`, `noq 1.2.0`, `iroh-tickets 1.0.0`,
`ed25519-dalek 3.0.0`, `ring 0.17.14`. **No `aws-lc-rs`** — `tls-ring` holds.

**`iroh-tickets` must be pinned to `1.0`, not `0.1`.** `0.1` depends on
`iroh-base 0.94`, which pins `ed25519-dalek =3.0.0-pre.1` and conflicts
irreconcilably with `iroh 1.1`'s `>=3.0.0-rc`. This was the plan's original
pin and it does not resolve. Already corrected in `f48dbc810`.

---

## Q1 — Does `presets::Minimal` contact zero third parties?

**Yes.** Proven at source level, corroborated on the wire.

`Builder::empty()` (iroh 1.1.0 `src/endpoint.rs:190`) is documented as *"an empty
builder with no address lookup services, and `RelayMode::Disabled`"*, and its
`transports` vector contains only `default_ipv4()` and `default_ipv6()` — no
`TransportConfig::Relay`. `Minimal::apply` (`src/endpoint/presets.rs:62`) then
sets **only** `crypto_provider` and nothing else. By contrast `N0::apply` is what
adds `PkarrPublisher::n0_dns()`, `PkarrResolver::n0_dns()`,
`DnsAddressLookup::n0_dns()` and `relay_mode(default_relay_mode())`.

Two residual contact paths were checked and are closed:

- **`NetReportConfig::default()` has `https_probes: true` and
  `captive_portal_check: true`.** Both operate against the *relay map*, which is
  empty under `Minimal`, so neither has a target. **Recommendation: set
  `NetReportConfig::minimal()` explicitly in `vox-mesh-transport` anyway** —
  defence in depth, so a future default change cannot silently reintroduce
  outbound HTTPS.
- **`PortmapperConfig::default()` is `Enabled` (UPnP/PCP/NAT-PMP, SSDP
  multicast).** But it is gated on iroh's `portmapper` *feature*, which is in
  `default`. Building with `default-features = false` — as the spike did —
  removes it entirely. **So the plan's "expect gateway UPnP/PCP chatter" does not
  apply to a `default-features = false` build: there is none.**

Wire corroboration: the listener's advertised `EndpointAddr` contained **only IP
addresses and no relay URL**:

```
EndpointAddr { id: PublicKey(fa6fcd64…71b1),
               addrs: {Ip(100.107.222.96:58828), Ip(172.20.160.1:58828),
                       Ip(172.24.80.1:58828), Ip(192.168.50.83:58828)} }
```

**Confirmed empirically 2026-09-05, without unplugging anything.** An earlier
revision of this page said the confirmation "requires disconnecting the router's
uplink, which is a hands-on act." That was asserted, not tested, and it is wrong.

A per-program Windows Firewall outbound rule on BLAPTOP04, applied over SSH
(that session is elevated), restricted `mesh_smoke.exe` to `192.168.50.0/24`
only — no internet, no tailnet. It dialled the Mac and completed a `Probe` in
**22.1 ms**. The rule shape was calibrated on `curl.exe` first (internet `000`,
`1.1.1.1` `000`, tailnet `000`, LAN `200`), and the negative control — the same
dial with *all* outbound blocked — fails with `timed out`, so the test is capable
of failing.

**One trap, worth more than the result.** Windows Firewall is **stateful**, so an
outbound block does **not** apply to the listener's replies on an established
inbound flow. Blocking the *listener* and watching the dial succeed reads as a
pass and proves nothing. Block the **initiator**; calibrate on a program you can
observe independently before trusting a reading from the one under test.

Corroboration from the Mac listener's socket census during the run: `UDP *:58535`,
`UDP *:64100`, `UDP *:5353` — three sockets, **zero remote addresses**.

### ROOT CAUSE FOUND — it was never `pf`, and it invalidates an earlier claim

**Windows blocks inbound LAN UDP to the mesh binary, so every macOS -> BLAPTOP04
connection had been silently falling back to the Tailscale tunnel.**

Measured with **no firewall active anywhere**, same peer, same second, one
explicit address each (`mesh_smoke dial-addr`, added precisely to remove the
ambiguity of a multi-address ticket):

| Path | Result |
|---|---|
| LAN `192.168.50.83` | **FAILED — 30 s timeout** |
| Tailnet `100.107.222.96` | **connected in 73 ms** |

BLAPTOP04 had **no inbound firewall rule** for the mesh binary, and Windows'
`NotConfigured` default inbound action is *block*. Tailscale is unaffected
because it delivers over `utun` after `tailscaled` has already accepted the
packet, bypassing per-application inbound rules. Adding one rule fixes it:

```powershell
New-NetFirewallRule -DisplayName vox-mesh-inbound-lan -Direction Inbound `
  -Action Allow -Program <mesh binary> -Protocol UDP `
  -RemoteAddress 192.168.50.0/24 -Profile Any
```

LAN path afterwards: **25.7 ms**.

**What this invalidates.** The earlier claim that the cross-machine demo ran
"no relay, no discovery service, direct LAN" was **half wrong**: the
BLAPTOP04 -> macOS direction did use the raw LAN, but macOS -> BLAPTOP04 was
riding Tailscale. The `pf` experiments then failed because blocking
`100.64.0.0/10` removed the *only working path* — so `pf` was correctly
blocking a route the connection genuinely depended on. Four hypotheses were
tested against the wrong subsystem.

**Why it took four attempts.** A ticket carries several candidate addresses and
iroh picks among them silently, so a ticket-based dial can never tell you which
path carried the connection. `dial-addr` exists so a result names its own path.
**A test that cannot report which mechanism it exercised cannot diagnose one.**

### The corrected offline result

With the inbound allow in place, and the mesh binary on BLAPTOP04 blocked
**inbound and outbound** from everything outside `192.168.50.0/24`:

| | |
|---|---|
| Negative control — tailnet path | **FAILED, 30 s timeout** (the block is real) |
| **Pure LAN path** | **connected in 12.3 ms, `Probed { host_triple: "x86_64-windows" }`** |

That is the offline requirement met for the listener, with a negative control
that fails correctly, driven entirely over SSH.

**A second self-inflicted false pass, caught.** The first version of this run
blocked only *outbound* on the listener and the tailnet negative control
**passed** — because, exactly as recorded earlier in this document, an outbound
block on a listener is inert; its replies ride established inbound flows. The
inbound block is what makes the claim true. The same mistake, twice, in one
investigation.

### ACTION REQUIRED — this is a product gap, not test scaffolding

`vox-mesh-inbound-lan` is **left in place on BLAPTOP04**, because removing it
breaks LAN meshing entirely. Nothing in the plan, the ADR, or `vox mesh`
creates or checks for it, which means **the mesh silently does not work on a
default Windows install** unless a VPN happens to paper over it. Options, in
preference order: have `vox mesh join` create the rule on Windows (elevation
required, so prompt for it); or have `vox doctor` detect its absence and print
the exact command. Until one lands, LAN meshing on Windows is undocumented
manual setup.

### The earlier macOS `pf` attempts, and why they are now explained

Four elevated runs with a `pf` LAN-only ruleset, each with a working negative
control (`1.1.1.1` `000`, `example.com` `000`, tailnet `000`, LAN router `200`)
and clean restore. **Every one timed out on the dial**, while the identical
ticket connected in **8–11 ms with `pf` off, immediately before and after.** So
`pf` was blocking the QUIC path; the mesh was never in question.

Hypotheses tested and eliminated, recorded so nobody repeats them:

| Attempt | Hypothesis | Outcome |
|---|---|---|
| v1 | — | **Invalid run.** The listener was started over SSH, and the `pf` block killed the tailnet, killing its child. Testing against nothing. |
| v2 | `block drop` blackholes the ticket's unreachable candidates so their probes hang past the connect deadline | Wrong — `block return` failed identically |
| v3 | pfctl stamps `flags S/SA` (a TCP-only match) on protocol-less rules, so UDP never matches | Wrong — explicit `proto udp` rules parsed correctly and still failed |
| v4 | Stale listener / wrong subnet | Wrong — listener alive (`pid 7700`), Mac still `192.168.50.208`, route direct on `en0` |

**RESOLVED.** None of these were the cause. `pf` was working correctly the whole
time: it blocked `100.64.0.0/10`, which was the only path that worked, because
Windows was dropping inbound LAN UDP. The guess recorded here previously — that
a machine-wide block disturbs `tailscaled` — was never tested and is not needed
to explain anything.

**This does not weaken the result.** The substantive claim — the mesh needs no
third party — is carried by the BLAPTOP04 run, which had a calibrated
instrument and a negative control that failed correctly. The macOS run would
have been confirmation of symmetry, not new information.

Still open, and cheap: both machines offline *simultaneously* at OS level. The
router is an ASUS at `192.168.50.1` with a reachable admin UI, so a per-client
block covers it without taking the household offline — which is what pulling the
uplink would have done, and without the `pf`-versus-Tailscale interaction above.

## Q2 — Does LAN connection succeed with no relay?

**Yes.** Mac → BLAPTOP04, direct, no relay, no discovery service:

```
connected in 12.773666ms to fa6fcd64db8d3fb1e1b52ecb22e7b004b12425a739aaaa554198eeed3ce871b1
rtt after handshake: Some(10.902208ms)
```

300 MiB transferred and acknowledged end-to-end. Ticket-only addressing.

## Q3 — Do the byte counters support a placement estimate?

**Yes, with a caveat, and the API surface is exactly as the plan described.**

`conn.stats() -> ConnectionStats` exposes `udp_tx`/`udp_rx`
(`{datagrams, bytes, ios}` — `ios` is `#[deprecated]` and always `0`),
`frame_tx`/`frame_rx`, `lost_packets`, `lost_bytes`. **No rtt and no cwnd.**
RTT is a separate first-class accessor, `conn.rtt(PathId) -> Option<Duration>`.

`cwnd` exists on `noq_proto::PathStats`, but iroh 1.1 does **not** re-export a
`path_stats()` accessor; the only route is
`conn.congestion_state(PathId) -> Option<Box<dyn Controller>>`, whose own doc
says *"for debugging purposes"*. **Do not build the cost model on `cwnd`** — that
closes off the BDP shortcut and leaves differentiation as the method.

Measured, 300 MiB Mac → Windows, sampling `udp_tx.bytes` every 200 ms:

| | |
|---|---|
| Ground truth (wall clock, sender) | **309.6 Mbit/s** |
| Ground truth (receiver-side timing) | 301.1 Mbit/s |
| Sampled mean of 39 windows | **320.6 Mbit/s** |
| Standard deviation | 46.8 Mbit/s |
| **Coefficient of variation** | **14.6 %** |
| Range | 205.8 – 417.0 Mbit/s |
| Loss over the transfer | 1 582 pkts / 1.89 MB of 325 MB (0.58 %) |

**Verdict: good enough.** The sampled mean lands within **3.6 %** of wall-clock
truth. Per-window noise of ±15 % is irrelevant to the decision the model
actually makes — "is shipping this worth it", a roughly 2× judgement, not a 5 %
one. **Recommendation:** use a window of ≥1 s, or an EWMA over the 200 ms
windows, to bring `cv` down before the figure reaches the model.

Observed RTT was noisy under saturation (10–53 ms on a LAN, versus 10.9 ms at
handshake) — classic bufferbloat. **Sample RTT when idle, not mid-transfer.**

## Q4 — mDNS discovery: ANSWERED, and the answer is that it does not work

**mDNS is wired, bound, and does not discover anything.** Tested directly with
`mesh_smoke dial-id`, which dials an `EndpointId` carrying **zero addresses** —
under `presets::Minimal` the only thing that can resolve that is mDNS, since
there is no relay, no DNS and no pkarr:

```text
dialing id ONLY, zero addresses (mDNS must resolve it): 93a10036…6693
FAILED after 10.005778125s: No addressing information available
```

Localised to the **announcing** side, not the lookup side, and not to either
platform:

| Observation | Result |
|---|---|
| `dns-sd -B _irohv1._udp` while a listener runs | **empty** |
| `dns-sd -B _ssh._tcp` (control, same method) | entries immediately |
| `dns-sd -B _services._dns-sd._udp` | `_companion-link`, `_smb`, `_rfb`, `_ssh`, `_sftp-ssh` on `en0` |
| Listener's own socket census | `UDP *:5353` **bound** |
| Windows listener, then macOS listener | **neither announces** |

So LAN multicast works, macOS's `mDNSResponder` sees plenty of services, our
process holds the mDNS port — and no `_irohv1._udp` record ever appears from
either machine. Eliminated by direct test: `advertise` defaults to `true`;
`AddrFilter::default()` is the identity filter; and removing our
`NetReportConfig::minimal()` line changed nothing.

**Leading hypothesis, explicitly untested:** `swarm-discovery` (the crate behind
`iroh-mdns-address-lookup`) implements its *own* mDNS stack on UDP 5353 rather
than registering with the host responder. On macOS `mDNSResponder` owns that
port, and iroh's own log line — `swarm_discovery::sender: no addresses for peer,
not announcing` — says the announcer believed it had nothing to publish. Anyone
resuming should start there, and should note this is a **third-party crate
pinned pre-1.0** (`=0.5.0`).

**Product consequence, which is the part that matters.** Discovery-by-`EndpointId`
does not work, so **ticket-based pairing is the only functioning path today**.
That is what `vox mesh join` already does, so the product is correct — but no
document may claim mDNS discovery works, and ADR-047 should not be read as
promising it.

## Q5 — Does `ep.online().await` hang under `Minimal`?

**Yes — it hangs forever. Confirmed at source level.**
`Endpoint::online` (`src/endpoint.rs:1358`) loops until some entry of
`home_relay_status()` reports `is_connected()`. That watcher is documented as
*"empty when no relays are configured"*, so under `RelayMode::Disabled` the
predicate is false on every iteration and `watcher.updated()` never delivers a
connected status. **Never put `online()` on the `Minimal` path**, exactly as the
plan warned.

## Q6 — Dependency weight and build time

**Marginal dependency weight: 55 crates.** Computed by differencing the spike's
resolved package set against the workspace's `Cargo.lock` (360 vs 1 447 packages;
55 in the spike are absent from the workspace). Headline additions: `iroh`,
`iroh-base`, `iroh-dns`, `iroh-relay`, `iroh-tickets`, `iroh-metrics`, `noq`,
`noq-proto`, `noq-udp`, `hickory-{proto,resolver,net}`, `netwatch`, `netdev`,
`n0-{error,future,watcher}`, the `netlink-*` family (Linux), `wmi`/`widestring`
(Windows). That is a **3.8 % increase** on a 1 447-package workspace.

**Build cost: 71.95 s wall, 245.83 s user + 24.03 s sys CPU**, clean release
build, `CARGO_INCREMENTAL=0`, sccache disabled, 1.3 GB peak RSS. This is an
**over-estimate of the marginal cost**: it compiles the spike's whole
360-package tree, including `tokio`, `serde`, `rustls` and `hyper`, which the
workspace already builds. **It is under the plan's ~90 s abort threshold, so
Task 0.3's conditional authorisation holds.**

Caveat on wall-clock numbers throughout: concurrent agent sessions were building
in sibling worktrees during this session. CPU-time figures are contention-robust;
wall-clock ones are not.

---

## Findings that change the plan

1. **`iroh-tickets = "0.1"` does not resolve.** Pin `1.0`. *(Already fixed.)*
2. **`ed25519-dalek` resolves to `3.0.0` stable**, not `-rc`. The ledger recorded
   the requirement as if it were the resolution. *(Already fixed.)*
3. **`default-features = false` removes the portmapper**, so there is no
   UPnP/SSDP traffic and no macOS firewall dialog to explain to users. Recommended
   feature set for `vox-mesh-transport`:
   `default-features = false, features = ["tls-ring", "fast-apple-datapath"]` —
   keeping `fast-apple-datapath` because it is in iroh's default set and is a
   macOS datapath optimisation the mesh wants.
4. **Set `NetReportConfig::minimal()` explicitly.** The defaults are safe today
   only because the relay map is empty.
5. **`cwnd` is debug-only.** The placement model must differentiate byte counters;
   there is no supported instantaneous-bandwidth accessor.
6. **A `finish()`ed stream is not a flushed stream.** The first spike revision
   dropped the `Connection` immediately after `send.finish()`, which closed the
   connection before the response reached the wire; the dialer saw
   `closed by peer: 0` and the payload was lost. `finish()` only signals
   end-of-stream. **The Phase 1 responder must await `conn.closed()` (or
   equivalent) before dropping the connection** — this is a silent data-loss bug,
   not a compile error, and it will recur in the real handler.
