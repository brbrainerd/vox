---
title: "Vox Hosting & Infrastructure Cost / ROI Audit (2026)"
description: "Point-in-time 2026 price audit and tiered own-vs-colocate-vs-rent advisory for hosting Vox compute, GPU serving, fine-tuning, control plane, and network, under a strict 24-month ROI hurdle."
category: "reference"
status: "research"
training_eligible: false
training_rationale: "Time-sensitive procurement/cost data, not durable architecture or product behavior."
---

# Vox Hosting & Infrastructure Cost / ROI Audit (2026)

> **Doc type:** Business / hardware-procurement planning reference — **not** an architecture
> doc and **not** an SSOT. It is a *dated cost snapshot* (June 2026) with a decision
> framework. Re-price before acting on any line item; GPU and colo markets move monthly.
>
> **Verification status (read this first).** The price spine was gathered by the
> `deep-research` workflow (5 search angles, 16 sources, 60 extracted claims). The
> workflow's adversarial-verify stage was **rate-limited out** (every vote returned
> `0-0 abstain`, which the harness *mislabels as "refuted"* — a known failure mode).
> The numbers below are therefore **single-source-gathered**, and the subset marked
> **✓ spot-checked** was independently re-confirmed via targeted fetches against a
> *second* source. Treat un-spot-checked figures as directional, not contractual.

---

## 1. The one-paragraph answer

**Under a 24-month ROI hurdle, buying new GPUs to serve or fine-tune Vox does not pay
off — cloud and flat-rate dedicated GPU prices have fallen far enough that rental wins
on every axis except "I already own it."** The only hardware purchases that clear the
2-year bar are (a) the workstation you **already own** (a sunk cost — keep using it for
Rust builds, local inference, and small LoRA fine-tunes; it already holds a Samsung 990
PRO 4 TB NVMe, so the originally-planned SSD buy is moot) and, optionally, (b) a single
~$160–200 2 TB NVMe for the **empty CPU-direct M.2_1 slot** as a dedicated Dev Drive to
isolate compile I/O. Everything incremental — public model serving,
large fine-tuning, CI overflow — should be **rented**: flat-rate dedicated (Hetzner GPU
servers, ~$200–$920/mo, with a real datacenter SLA) for steady baseline load, and
**spot** marketplaces (Vast.ai / RunPod, $0.20–$1.55/GPU-hr) for bursts. **Colocation is
a later-stage move**, justified only once you own ≥3–4 GPUs running >70% utilization
24/7 *and* need an SLA a home/apartment cannot provide.

---

## 2. Existing assets (sunk cost — exploit before spending)

| Asset | Spec | Best role | Marginal cost |
|---|---|---|---|
| **Local workstation** | i9-14900KS (8P+16E, 32 threads), RTX 4080 Super 16 GB, 64 GB DDR5 | Rust dev + fast incremental compile + local quantized LLM inference + LoRA/QLoRA fine-tunes ≤16 GB | **$0** (owned) |
| **Hetzner VPS** | ~$28/mo (you stated) | Control plane / API gateway / WireGuard hub / TLS termination / website | $28/mo (already paid) |
| **voxlang.org** | Domain + site | Public marketing/docs front door | ~$0 incremental |
| **Fableforge (Horst)** | Existing capacity | Secondary control-plane / build helper / staging — confirm specs before loading it | unknown — **open question** |

The workstation is the single most valuable asset in this whole analysis: a 32-thread
i9-14900KS is already a top-tier **Rust compile box**, and the RTX 4080 Super already runs
12–14 GB quantized models at 60–140 tok/s locally. Neither needs replacement. The one
defensible upgrade is storage (below).

---

## 3. Workload-by-workload need (where each resource actually matters)

| Workload | Bottleneck | Owned-asset fit | Buy / rent verdict |
|---|---|---|---|
| **(a) Rust compilation / CI** | CPU cores + NVMe random I/O (parallel codegen units; single-threaded link) | i9-14900KS is ideal; only the SSD is a gap | **Own** (already do). Overflow → cheap rented CPU dedicated, not new metal |
| **(b) GPU fine-tuning** | VRAM + GPU-hours, bursty | 4080 Super handles LoRA ≤16 GB; bigger needs 1–8× A100/H100 | **Rent spot** for anything >16 GB. Never own large-VRAM cards for 2-yr ROI |
| **(c) GPU serving to public** | VRAM + sustained throughput + **uptime/SLA** | 4080 Super 16 GB cannot safely face multi-user public traffic (context thrash → CPU offload → 3–11× slowdown) | **Rent flat-rate** dedicated baseline + **spot** burst |
| **(d) Control plane / website** | Always-on, low CPU, public IP, durability | Hetzner VPS already does this | **Keep** the $28/mo VPS |
| **(e) Network / inter-node** | Bandwidth + latency between GPU ↔ control plane ↔ web; NAT traversal; durability | VPS as WireGuard hub; GPU node dials out | **Rent**; co-locate GPU and gateway in the *same provider region* to kill cross-host latency |

**Network design that falls out of this:** public ingress → Hetzner VPS (TLS, auth,
rate-limit, Rust/Axum reverse proxy) → **WireGuard tunnel** (`PersistentKeepalive=25`,
outbound-initiated so the GPU node needs no inbound ports) → GPU node (vLLM/SGLang on a
private IP). This keeps the GPU backend off the public internet regardless of whether it
is a Hetzner GPU server, a rented spot box, or (later) colocated metal — the gateway
contract doesn't change as the backend moves. **Latency caveat:** if the gateway is in
Germany (Hetzner) and the GPU node is in a US cloud, you eat a ~90–150 ms transatlantic
RTT on every request. Put the gateway and the GPU node in the **same region** (both
Hetzner EU, or both US) to keep added hop latency in the single-digit-ms range.

---

## 4. Price spine (June 2026)

### 4.1 Consumer / prosumer GPUs (street prices) — ✓ spot-checked

| Card | VRAM | New (street) | Used | MSRP | Note |
|---|---|---|---|---|---|
| RTX 5090 | 32 GB GDDR7 | **~$4,329** | ~$3,999 | $1,999 | ~2.2× MSRP; used barely discounts |
| RTX 4090 | 24 GB GDDR6X | **~$2,755** | ~$2,349 | $1,599 | still ~1.7× MSRP years post-launch |
| RTX 3090 | 24 GB GDDR6X | — | **~$700–1,050** | $1,499 | best $/VRAM for owned local inference |
| RTX 4080 Super | 16 GB | *(owned)* | — | $999 | your card — sunk cost |

*Sources: bestvaluegpu.com 5090/4090/3090 trackers; xda-developers 3090 used.*

### 4.2 Cloud GPU rental ($/GPU-hr) — ✓ spot-checked

| GPU | Spot / marketplace | On-demand | 24/7 monthly (730 hr) @ low end |
|---|---|---|---|
| RTX 4090 (24 GB) | $0.20–0.34/hr (RunPod community, Vast) | ~$0.44/hr | **~$146–248/mo** |
| A100 80 GB | $0.60–0.67/hr (Spheron/Vast spot) | ~$1.07/hr | **~$438/mo** |
| H100 80 GB | $1.03–1.19/hr spot | $1.50–1.55/hr (RunPod/Vast) | **~$752/mo** |
| B200 | from ~$2.12/hr | — | — |

*Sources: Spheron 2026 GPU pricing, Northflank, computeprices.com, the velinxs/Medium
Vast-vs-RunPod 2026 comparison.* **Vast.ai is usually cheapest** (marketplace, requires
fiddling); **RunPod** is click-and-go and occasionally matches/beats on H100.

### 4.3 Flat-rate dedicated GPU servers (Hetzner) — ✓ spot-checked

| Server | GPU | VRAM | Host | Monthly | Setup |
|---|---|---|---|---|---|
| **GEX44** | RTX 4000 SFF Ada | 20 GB | i5-13500, 64 GB DDR4, 2×1.92 TB NVMe | **€184 (~$200)** | €79 |
| **GEX130** | RTX 6000 Ada | 48 GB | Xeon Gold 5412U, 128 GB DDR5 ECC, 2×1.92 TB Gen4 NVMe | **€838 (~$920)** | €79 |

*Source: hetzner.com GEX44 / GEX130 pages.* These include datacenter power, cooling,
bandwidth, and an SLA — i.e. all the costs a home GPU node hides.

### 4.4 Colocation (US, Phoenix/Tucson focus) — gathered, not spot-checked

- **Phoenix GPU/AI colo:** ~$130–275 /kW/mo (single-rack to hot-rack); among the cheaper
  major SW-US markets vs Silicon Valley ($200–350) / N. Virginia ($180–300).
- **Wholesale primary-market average:** ~$196 /kW/mo (H2 2025, +6.6% YoY).
- **Retail per-U (Colocation America style):** ~$75/mo 1U · ~$399/mo 10U · ~$999/mo 42U.
- **Power often billed separately** ("+E" pass-through) on top of the $/kW space rate.

### 4.5 Arizona power & the home-hosting tax — gathered, not spot-checked

- **Residential power:** Phoenix ~$0.15/kWh, Tucson ~$0.17/kWh (APS fixed 13–17¢; TOU 4–49¢).
- **Home GPU node true cost:** a ~500 W 24/7 node ≈ 365 kWh/mo ≈ **$55/mo power**, plus
  **AZ summer cooling** (effective PUE ~1.3–1.4) adds ~$15–20/mo — call it **~$70/mo
  all-in per node**, *before* the lease-violation / noise / single-utility-feed / no-SLA
  risks of running servers in an apartment.

### 4.6 Storage — measured machine state (updated 2026-06-20)

**A terminal hardware audit changed this line item from "buy" to "optimize what's
owned."** Current state of the workstation (MSI Z790 GAMING PLUS WIFI, MS-7E06, BIOS H.90):

| Item | Measured | Implication |
|---|---|---|
| **Samsung 990 PRO 4 TB** | Present, **PCIe Gen 4 ×4** (max Gen4 ×4), healthy | Already own the recommended drive — at 2× the planned capacity. **No NVMe purchase needed.** |
| Its slot | Parent = Intel **PCH Root Port #25 (7A48)** → **chipset-attached** M.2 slot (M.2_2/3/4) | Full Gen4 ×4, but routes through the shared **DMI 4.0 ×8** link (contends with the 2× SATA HDDs, USB, WiFi, LAN) |
| **M.2_1 (CPU-direct)** | **Empty** | The optimal low-latency slot is free |
| 2× WD Red Pro 16 TB | SATA → **29.8 TB RAID0** (X:), 8.3 TB free | Bulk / model cache lives here, not on C: |
| C: (OS, on the 990 PRO) | NTFS, **557 GB free / 3,725 GB** | Filling up; keep models off C: |
| **Dev Drive** | **None exists** (C:/X: both NTFS; cargo cache on default `~/.cargo` = on the NVMe) | The plan's compile speedup (ReFS + deferred Defender) is **not yet applied** — this is the real remaining win |

**Decision (chosen 2026-06-20): use the empty M.2_1 (CPU-direct) slot for a dedicated
Dev Drive / build volume.** Preferred path **B2** — add a **new 2 TB NVMe** (Samsung 990
PRO 2 TB / WD SN850X 2 TB, ~$160–200 on sale) in M.2_1, leave the OS on the existing 4 TB.
This isolates build I/O on CPU-direct lanes from OS/HDD I/O on the DMI link — the cleanest
compile setup and the one remaining purchase that clears the 2-yr ROI bar (kills daily
compile-wait friction on a 116-crate workspace). Free fallback **B1**: relocate the
existing 4 TB into M.2_1 and carve the Dev Drive there ($0, but OS+build share one drive).
**Do not** buy a Gen 5 drive — this board has no Gen 5 M.2 slot. After install: format the
M.2_1 volume as a **ReFS Dev Drive**, move the repo + point `CARGO_HOME` at it.

### 4.7 Uptime/SLA reference — gathered

99.9% = ~8h45m downtime/yr · 99.95% = ~4h22m · 99.99% = ~52m · 99.999% = ~5m. A home
apartment realistically delivers **<99.5%** (single utility feed, residential ISP with no
binding SLA, consumer AC). A Hetzner dedicated / Tier-2+ colo delivers 99.9–99.99%.

---

## 5. The 24-month ROI verdict (own vs rent vs colo)

The decisive comparison is **a 24/7 GPU-serving node**, amortized over 24 months:

| Option | Capex | Monthly run cost | **All-in /mo over 24 mo** | SLA | Notes |
|---|---|---|---|---|---|
| **Own home RTX 4090 node** | ~$3,800 (used 4090 + box) | ~$70 power+cooling | **~$228** | **none** (<99.5%) | barely beats on-demand cloud; *loses* to flat-rate |
| **Rent RunPod 4090 24/7** | $0 | $248 (on-demand) / $146 (spot) | **$146–248** | provider | no hardware/uptime risk |
| **Hetzner GEX44 (Ada 20 GB)** | €79 | €184 | **~$203** | **datacenter** | flat, managed, SLA — *beats owning* |
| **Own home 5090 node** | ~$5,800 (new 5090 + box) | ~$75 | **~$317** | none | worst ROI; 5090 at 2.2× MSRP destroys the math |

**Conclusions:**

1. **Serving:** A home GPU node's all-in cost (~$228–317/mo) is **higher** than a
   flat-rate Hetzner GPU server (~$203/mo) that *includes* datacenter power, cooling,
   bandwidth, and an SLA. Owning fails the ROI test for serving. **Rent flat-rate.**
2. **Fine-tuning large models:** an 8×H100 box is **$250k+** capex; renting at ~$1.03/hr
   spot = ~$8.24/hr for the 8-GPU set. You'd need **~30,000 GPU-hours in 24 months**
   (~58 hrs/week non-stop) just to amortize the *cards*. Implausible for a bootstrapping
   project. **Rent spot, always.** (Your inbound report's 5-node / 30× RTX 5090 cluster
   for a 1.6 T model = ~$66k in cards alone — explicitly rent-only, not a 2-yr-ROI build.)
3. **The only "own" that clears 2-year ROI** is the **already-owned workstation**
   (sunk) plus the **~$300 NVMe** upgrade (pays for itself in saved compile-wait within
   weeks). Buying a *new* GPU for this project does **not** clear the bar in 2026.
4. **Colocation** only becomes rational at the production-SLA tier (§6.3): when you own
   ≥3–4 GPUs you must keep >70% utilized 24/7 *and* need 99.9%+ that an apartment can't
   give. Below that scale, flat-rate dedicated hosting dominates colo on total cost
   *and* operational burden.

**The break-even intuition to remember:** owning a GPU beats renting it only when you can
hold it at **high sustained utilization for the full 2 years**. Bursty research/fine-tune
load = rent spot. Steady baseline serving = rent flat-rate. Continuous, predictable,
high-volume serving at scale = the *only* place owning/colo starts to win — and you're not
there yet.

---

## 6. Tiered recommendations

### 6.1 Bootstrap tier (now — minimize cash, ≤~$80/mo new spend)

- **CPU/Rust/CI:** owned workstation. **NVMe already owned** (990 PRO 4 TB) — *no $300
  buy.* Optional one purchase: a **2 TB NVMe (~$160–200) for the empty M.2_1 CPU slot** as
  a dedicated ReFS **Dev Drive** / build volume → move repo + `CARGO_HOME` there. Add
  `mold`/`lld` linker. That's the entire hardware budget (and it's optional).
- **Local inference + small fine-tunes:** owned 4080 Super (Q4/Q8 ≤14 GB models).
- **Serving (if any public demand):** start on **spot** (Vast.ai/RunPod RTX 4090
  $0.20–0.34/hr) — pay only when serving. Or a single **Hetzner GEX44 ~$200/mo** if you
  need always-on.
- **Control plane + web:** existing **$28/mo Hetzner VPS** + voxlang.org. Add WireGuard.
- **New monthly spend: ~$0–28** (plus the one-time NVMe). **Own nothing new but the SSD.**

### 6.2 Growth tier (real but spiky public traffic)

- **Serving baseline:** one **Hetzner GEX44 (~$200/mo)** or **GEX130 (~$920/mo)** for
  steady load behind the VPS gateway (same region → low latency).
- **Burst:** autoscale to **RunPod/Vast spot** during spikes; cap with the gateway's rate
  limiter.
- **Fine-tuning:** rent **A100/H100 spot** per-run ($0.60–1.19/hr); never own.
- **CI overflow:** cheap **Hetzner CPU dedicated (AX-line)** or the existing self-hosted
  fleet — not new desktop metal.
- **Still own nothing new.** All elastic, all SLA-backed, all sub-$1k/mo until volume
  justifies more.

### 6.3 Production-SLA tier (sustained, high-volume, contractual uptime)

- **Trigger to consider owning + colocating:** you can keep **≥3–4 GPUs >70% utilized
  24/7** for 2+ years, *and* a 99.9%+ SLA is a business requirement.
- **Then:** build enterprise-class nodes (Threadripper PRO / EPYC for full ×16 PCIe lanes
  per GPU — *not* consumer AM5/Z790, which bifurcates lanes and bottlenecks tensor-parallel
  all-reduce) and **colocate in Phoenix** (~$130–275/kW/mo + ~$0.15/kWh power) for 2N
  power, N+1 cooling, and blended-carrier BGP. Phoenix is the cheaper SW-US market.
- **Do not run this in an apartment.** Residential hosting saves ~$460/mo vs colo but
  delivers <99.5% uptime, risks lease termination, and gambles ~$20k+ of hardware on a
  single consumer AC unit in 115 °F summers. The colo delta is an insurance premium, not
  a luxury, once uptime is contractual.
- **Keep the gateway split:** public ingress stays on the cheap VPS; colocated GPUs stay
  private behind WireGuard. The architecture from §3 scales unchanged.

---

## 7. What to do with the named assets

- **Workstation (i9-14900KS / 4080 Super):** primary dev + compile + local inference +
  LoRA. Buy the NVMe; otherwise leave as-is. Do **not** repurpose it as a public-serving
  node (16 GB VRAM thrashes under concurrent users → CPU offload → 3–11× slowdown).
- **Hetzner VPS ($28/mo):** promote to the **always-on control plane**: Rust/Axum reverse
  proxy, TLS, API-key auth, rate limiting, WireGuard hub. It is the durable public face;
  GPU backends come and go behind it.
- **voxlang.org:** static site / docs / marketing on the VPS (or a CDN). No GPU needed.
- **Fableforge (Horst):** **open question — get its specs.** If it's a capable always-on
  box, it can be a *second* control-plane/CI/staging node or a WireGuard peer, deferring
  any new CPU rental. If it has a usable GPU, benchmark it for baseline serving before
  paying Hetzner. Don't assume; measure.

---

## 8. Open questions / re-verify before committing

1. **Re-price at purchase time.** GPU street and cloud-hr prices move monthly; the colo
   and AZ-power numbers here are **single-source, not spot-checked**.
2. **Fableforge specs + uptime** — the biggest unknown that could remove a line item.
3. **Actual public traffic shape** — without a demand curve, "spot vs flat-rate baseline"
   can't be tuned. Instrument the gateway first; size GPUs from real concurrency.
4. **Region pairing** — confirm gateway and GPU node share a region to avoid the
   transatlantic-RTT trap if the VPS stays in Germany.
5. **Colo quotes are list, not negotiated** — get real Phoenix quotes (Login DC2,
   phoenixNAP, Iron Mountain AZS-1) only at the production-SLA tier.

---

## 9. Sources

Gathered (June 2026); **✓** = independently spot-checked during synthesis.

- ✓ Vast.ai vs RunPod 2026 pricing — medium.com/@velinxs
- ✓ Spheron / Northflank / computeprices.com — 2026 cloud GPU rates
- ✓ bestvaluegpu.com — RTX 5090 / 4090 / 3090 trackers; xda-developers (3090 used)
- ✓ hetzner.com — GEX44 / GEX130 dedicated GPU server specs & pricing
- quotecolo.com (Phoenix / single-rack AI colo); datacenterhawk.com; brightlio.com — colo $/kW
- energysage.com (Phoenix/Tucson power); tep.com 2026 rates — AZ electricity
- cushmanwakefield.com — data-center lease/power outlook
- inmotionhosting.com — SLA downtime-budget reference
- tomshardware.com — SSD price tracking (NVMe)
