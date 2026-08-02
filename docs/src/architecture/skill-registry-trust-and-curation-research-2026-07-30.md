---
title: "Skill/Tool Registry Curation & Trust Models — Verified Research 2026-07-30"
description: "Adversarially verified research into the official MCP Registry's namespace-ownership and moderation model: DNS/GitHub-verified publishing, a deliberately permissive takedown policy that never removes vulnerable servers, and mutable-status soft-delete revocation — the closest published prior art for a Vox skill registry."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Skill/Tool Registry Curation & Trust Models — Verified Research (2026-07-30)

> **Provenance.** `deep-research` run `wf_d875e11c-99d`, closing the coverage gap the induction
> research (batch 3) explicitly flagged as unresearched. **91 of 106 agents completed** before
> hitting a session usage limit (reset 1:20am ET) that killed the synthesis stage and 15
> in-flight verifiers; the run returned its **20 confirmed / 1 refuted / 4 unverified** claims
> unmerged rather than losing them. Read directly from the raw claim list below — no
> re-synthesis was needed because the individual claims are already source-quoted and voted.
>
> **What's solid:** everything on the **official MCP Registry** (§1) — 17 of 20 confirmed
> claims, all sourced to `modelcontextprotocol.io` or the registry's own GitHub repo, mostly
> **3-0**. **What's thin:** Claude Code's marketplace model (§2) — 4 claims about it never
> finished their verification votes (killed by the session limit) and are reported as
> plausible-but-unverified, not confirmed. **What's missing entirely:** (b) GPT Store/Actions
> policy, (d) MCP security incidents/CVEs, and (e) npm/crates.io/PyPI provenance prior art —
> the run was killed before reaching those angles. This document is a **solid first half**, not
> the full research question.

Companion: [`skill-discovery-and-induction-research-2026-07-30.md`](skill-discovery-and-induction-research-2026-07-30.md)
§7 (the gap this closes). Feeds: [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md).

---

## 0. The one-paragraph answer

The official MCP Registry solves exactly one problem — **namespace ownership** — and solves it
well, via reverse-DNS names cryptographically tied to a verified GitHub account or domain. It
explicitly, repeatedly, and in writing **declines to solve every other trust problem**: no
security scanning, no vulnerability-based takedown, "minimal-to-no moderation" by its own
wording, and a takedown policy so narrow that **a server with a known, disclosed security
vulnerability is not removed.** Every value-added trust signal — ratings, scanning, curation —
is explicitly pushed downstream to aggregators. **This is a layered-trust architecture, not a
gatekept one**, and it is the opposite of what many casual descriptions of "the MCP registry"
imply.

---

## 1. The official MCP Registry — what it actually is

### 1.1 Namespace ownership (confirmed, 2-1 / 3-0)

Server names are **reverse-DNS**, tied to a verified identity:

> "Server names follow a reverse DNS format (like `io.github.username/server` or
> `com.example/server`) that ties them to verified GitHub accounts or domains. This namespace
> system ensures that only the legitimate owner of a GitHub account or domain can publish
> servers under that namespace."

Enforcement at publish time, worked example from the registry's own repo:

> "you must login to GitHub as `domdomegg`, or be in a GitHub Action on domdomegg's repos …
> you must prove ownership of `adamjones.me` via DNS or HTTP challenge"

**Three distinct publisher authentication mechanisms**, not one account model (confirmed 3-0):

| Mechanism | Use case |
|---|---|
| **GitHub OAuth** | interactive publishing by logging into GitHub |
| **GitHub OIDC** | CI-based publishing from GitHub Actions |
| **DNS / HTTP verification** | proving ownership of a domain and its subdomains |

**This is the single most transplantable piece of prior art for a Vox skill registry.**
Reverse-DNS naming + GitHub-or-domain-verified publishing is cheap to implement, has no
central-authority bottleneck, and is exactly the shape Vox's own `io.github.<user>/<skill>`
convention could adopt directly.

### 1.2 No code scanning — explicitly delegated (confirmed 3-0)

> "The MCP Registry delegates security scanning to: **Underlying package registries** …
> **Downstream aggregators** … The MCP Registry focuses on namespace authentication and
> metadata hosting, while relying on the broader ecosystem for security scanning of actual
> server code."

The registry is a **metadata catalog, not a code host** (confirmed 3-0): *"a list of MCP
servers, like an app store for MCP servers."* Because server implementations and distribution
remain the publisher's responsibility, **registry-level controls cannot by themselves prevent
post-publish mutation of the underlying package or endpoint** — a namespace-verified publisher
can still ship a malicious update tomorrow, and the registry has no mechanism to catch it.

### 1.3 "Minimal-to-no moderation" — stated as policy, not as a gap (confirmed 3-0)

The registry's own moderation policy, verbatim, twice, in two documents:

> "The MCP Registry **does not** make guarantees about moderation, and consumers should assume
> minimal-to-no moderation."

> "we largely rely on upstream package registries (like NPM, PyPI, and Docker) or downstream
> subregistries (like the GitHub MCP Registry) to do more in-depth moderation."

### 1.4 The takedown policy — and what it deliberately does NOT cover (confirmed 3-0, twice independently)

**Removed:**

> "Illegal content, which includes obscene content, copyright violations, and hacking tools" ·
> "Malware, regardless of intentions" · "Spam, especially mass-created servers that disrupt the
> registry" (including marketing-stuffed descriptions with unrelated implementations) ·
> "Non-functioning servers"

**Explicitly NOT removed — quoted verbatim, and this is the finding to sit with:**

> "we therefore **won't** remove: - Low-quality or buggy servers - Servers with **security
> vulnerabilities** - Servers that do the same thing as other servers - Servers that provide or
> contain adult content"

**A server with a disclosed, known security vulnerability stays listed.** This is not an
oversight — it is a stated, deliberate policy choice, corroborated across two independent pages
in the registry's own documentation. The registry's implicit position is that vulnerability
triage is a downstream/consumer responsibility, not a registry-gate responsibility.

### 1.5 Revocation — mutable status, soft delete, no propagation guarantee (confirmed 3-0)

Metadata is otherwise immutable except one field:

> "Server metadata is generally immutable, except for the `status` field which may be updated
> to, e.g., `\"deprecated\"` or `\"deleted\"`. We recommend that aggregators keep their copy of
> each server's `status` up to date."

Note the word: **recommend**. Propagation to downstream indexes is advisory, not enforced.

When a server *is* removed for cause:

> "When we remove a server, we set the server's `status` to `\"deleted\"`, but the server's
> metadata remains accessible via the MCP Registry API. Aggregators may then remove the server
> from their indexes. In extreme cases, we may overwrite or erase the server's metadata."

**Soft-delete by default; hard erasure only in extreme cases** (e.g. unlawful metadata). The
`io.modelcontextprotocol.registry/official` provenance marker on a listing is **registry-added
and read-only** — a publisher cannot self-assert official-registry provenance in a submitted
`server.json` (confirmed 3-0).

### 1.6 The registry is explicitly not authoritative or always-available (confirmed 3-0)

> "The MCP Registry provides an unauthenticated read-only REST API that aggregators can use to
> populate their data stores. Aggregators are expected to scrape data on a regular but
> infrequent basis (e.g., once per hour), and persist the data in their own data store. The MCP
> Registry **does not provide uptime or data durability guarantees**."

**Consequence:** any consumer treating the official registry as a live source of truth is
building on an explicitly disclaimed foundation. The correct integration pattern — which
Smithery and PulseMCP both implement — is scrape-and-cache, not query-on-demand.

---

## 2. The layered-trust model, made explicit

Reading §1.1–1.6 together, the MCP Registry's design is coherent once named:

```
IDENTITY LAYER    (the official registry — namespace ownership only)
     │  reverse-DNS name ←→ verified GitHub account or domain
     │  no code review, no vuln gate, "minimal-to-no moderation"
     ▼
CURATION LAYER    (aggregators — Smithery, PulseMCP, GitHub MCP Registry)
     │  ratings, security scanning, deeper review — all EXPLICITLY pushed here
     ▼
DISTRIBUTION LAYER (npm / PyPI / Docker — the underlying package registry)
     │  the actual code-scanning delegate named in §1.2
     ▼
ENTERPRISE LAYER   (Claude Code's marketplace allowlist/blocklist — see §3)
```

**This is a defensible design, not a weak one** — it correctly recognizes that a single
centralized authority cannot simultaneously verify identity cheaply *and* review code
thoroughly *and* stay available at scale. It separates concerns. **The failure mode is only in
consumers who mistake "listed in the official registry" for "vetted."** The registry's own
documentation goes out of its way, in at least four separate places, to prevent that
misunderstanding — which is itself worth copying: **say the disclaimer where the trust
decision gets made, not only in a FAQ.**

---

## 3. Claude Code's marketplace model — unverified, reported honestly

Four claims about Claude Code's own plugin-marketplace trust model were extracted and **never
completed their verification vote** when the session limit killed the run. They are
**plausible and internally consistent with the MCP Registry findings above**, but they are
**not confirmed** and must not be cited as established fact:

- Claude Code reportedly reserves a fixed list of marketplace names for Anthropic and blocks
  impersonating variants — client-side denylist, not registry-side namespace ownership.
- The reserved-name check reportedly re-runs on every load, so an already-installed
  marketplace can stop loading retroactively if a name collision is later detected.
- There is reportedly **no central review, signing, or provenance attestation** in the model —
  a marketplace is a self-published `.claude-plugin/marketplace.json` in any git repository.
- Trust enforcement is reportedly delegated to **enterprise policy** rather than the client or
  registry — administrators allowlist/blocklist via `strictKnownMarketplaces` /
  `blockedMarketplaces`.

**If accurate**, this is the same layered pattern as §2, with the enterprise-policy layer
substituting for a curation layer. A follow-up pass should re-verify these four specifically
against `code.claude.com/docs` before Vox's plan cites them.

---

## 4. What this means for a Vox skill registry

Direct application of §1–2 to the audit's finding **F8** (no `skill_candidates` table, mined
skills have nowhere to persist) and **F13** (the miner has one manual caller):

1. **Copy the namespace model exactly.** Reverse-DNS or `io.github.<user>/<skill>` naming, tied
   to GitHub OAuth/OIDC or DNS verification. This is cheap, decentralizes trust, and has
   working prior art to fork rather than design from scratch.
2. **Do not conflate "listed" with "vetted."** Vox's promotion gate (induction doc §4) already
   does more validation than the MCP Registry attempts — execution gate, independent verify,
   generality gate, shadow period. **That gate belongs at the identity layer for Vox, not
   pushed downstream**, precisely because Vox controls the whole stack end-to-end in a way the
   MCP ecosystem's multi-vendor registry does not. This is a place Vox can be stricter than the
   prior art, not just copy it.
3. **A vulnerability disclosure MUST gate a skill, unlike the MCP Registry's stated policy.**
   §1.4 is defensible for a decentralized multi-party registry; it is **not** defensible for a
   single-vendor skill library feeding directly into an agent's tool-use loop. Vox's
   `skill_reliability` table is exactly the substrate for a stricter policy: sustained failure
   or a flagged vulnerability should force `status: deprecated`, propagated immediately (not
   "recommended") because Vox controls every downstream consumer.
4. **Adopt mutable-status soft-delete, not hard erasure**, for the same reasons the MCP
   Registry does — audit trail, no silent history rewrite, aggregator (in Vox's case: any
   cached GUI catalog) can reconcile on next sync.
5. **State the trust boundary at the point of use.** The MCP Registry's repeated,
   multi-document disclaimer is worth copying almost verbatim into Vox's skill-install UI:
   *installed does not mean vetted; here is what was actually checked.*

---

## 5. Refuted and unverified — do not restate

**Refuted (0-3):** the claim that, as of a 2025-09-29 schema revision, the `status` field was
removed from the publisher-controlled `server.json` and made registry-managed in API responses.
Did not survive verification.

**Unverified, not confirmed** — all four Claude Code marketplace claims in §3. Report as
"reportedly," never as established.

---

## 6. Open questions — what this run did not reach

The session limit killed the run before these angles were researched at all:

1. **OpenAI GPT Store / Actions review and takedown policy** — zero coverage.
2. **MCP security research**: tool-poisoning, rug-pull attacks, tool-description prompt
   injection, name-squatting incidents, and any CVEs. Zero coverage. This is the sub-question
   most directly relevant to whether Vox's registry design needs an active-attack threat model
   beyond the passive-trust model covered here.
3. **npm / crates.io / PyPI provenance prior art** — sigstore, npm provenance attestations,
   post-publish revocation mechanics. Zero coverage, despite being explicitly requested as the
   most mature comparable ecosystem.
4. **Re-verification of the four Claude Code marketplace claims in §3.**

These four gaps are the natural scope for a follow-up research batch.
