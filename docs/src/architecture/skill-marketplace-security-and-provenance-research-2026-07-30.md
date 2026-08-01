---
title: "Skill Marketplace Security: MCP Attacks, GPT Store Review, Package Provenance — Verified Research 2026-07-30"
description: "Adversarially verified research closing the security half of the registry-trust gap: named, PoC-demonstrated MCP attack classes (Tool Poisoning, Rug Pull, Advanced Tool Poisoning), a large-scale census finding the MCP ecosystem 'rife with exploitable gadgets,' OpenAI's GPT Store review process against independent research showing 95%+ of listed GPTs remain exploitable, and npm/crates.io's concrete anti-squatting and Sigstore provenance prior art."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Skill Marketplace Security: Attacks, Review Policy & Provenance (2026-07-30)

> **Provenance.** `deep-research` run `wf_013086e2-714`: **102 of 106 agents completed cleanly**
> (4 errored on transient rate-limiting, not content issues — see below), **9 confirmed
> findings, all high-confidence, all 3-0 or aggregate 3-0 across sub-claims.** This closes the
> security half of the gap flagged in
> [`skill-registry-trust-and-curation-research-2026-07-30.md`](skill-registry-trust-and-curation-research-2026-07-30.md)
> §6 — that document covered the MCP Registry's *identity/moderation* model; this one covers the
> *attacks the identity model doesn't stop* and the *concrete provenance prior art* to close
> that gap.
>
> **This is the most operationally important document in the research set for anyone shipping a
> Vox skill registry.** The MCP Registry doc (companion, §1.4) already established that
> vulnerability disclosure does not trigger delisting. This document establishes **why that
> matters concretely**: the vulnerability classes are named, demonstrated with working PoCs, and
> — per the large-scale census in §2.3 — systemic, not edge-case.

Companions: [`skill-registry-trust-and-curation-research-2026-07-30.md`](skill-registry-trust-and-curation-research-2026-07-30.md),
[`skill-discovery-and-induction-research-2026-07-30.md`](skill-discovery-and-induction-research-2026-07-30.md)
§4 (the promotion gate this hardens). Feeds: [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md)
Phase 3.

---

## 0. The headline finding

**Review processes that check descriptions at listing time do not stop the attacks that matter,
because the attacks specifically target the gap between what's reviewed and what runs.** Three
independent lines of evidence converge on this: MCP tool-poisoning attacks work because the
*model* sees the full tool description while the *user-facing UI* shows a simplified version
(§1.1); "rug pull" attacks work because review happens once, at install, and the tool can change
after (§1.1–1.2); and OpenAI's GPT Store — which *does* run combined human+automated review with
brand-impersonation classifiers — still has a **95%+** exploitability rate in independent
academic testing (§3). **A registry cannot solve this by reviewing harder. It has to solve it by
narrowing what a reviewed artifact is allowed to do after review**, which is a capability/sandbox
problem, not a moderation problem.

This directly validates the induction doc's promotion-gate design (induction doc §4) and adds
one gate to the parity plan's Phase 3 pipeline that wasn't there before: **re-verification on
tool-behavior change, not just on initial promotion** — see §5.

---

## 1. MCP security incidents — named, dated, demonstrated (confirmed, high)

### 1.1 Invariant Labs (April 2025) — Tool Poisoning, Tool Shadowing, sleeper Rug Pull

Confirmed via direct fetch of both the GitHub PoC repo and the primary blog post, with matching
verbatim quotes across five separate verification passes:

- **Tool Poisoning Attack**: a malicious instruction is embedded in a tool's *description* —
  invisible to the user, fully visible to the model. The exploit mechanism, stated plainly:
  **AI models see the full tool description (including hidden instructions) while user-facing
  UIs show only a simplified version.** This is not a bug in one client; it is structural to how
  tool descriptions are consumed.
- **Tool Shadowing**: a malicious tool's description instructs the model to alter the behavior
  of a *different, legitimate* tool — demonstrated with an email-exfiltration PoC.
- **"Sleeper" Rug Pull**: an MCP server that behaves benignly on first load (passing whatever
  review or trust check occurs at install time) and **swaps its tool interface to malicious only
  on the second load** — specifically designed to defeat point-in-time review.
- **Working proof-of-concept against Cursor**: leaking `mcp.json` and **SSH keys**. A separate
  demonstrated example exfiltrates WhatsApp messages.

### 1.2 CyberArk — Advanced Tool Poisoning Attacks (ATPA) and named "MCP Rug Pull"

Confirmed via direct fetch, concrete SSH-key exfiltration example matching almost verbatim:

- **ATPA's innovation over the original Tool Poisoning Attack**: it poisons the tool's
  **runtime output** (e.g., a fabricated error message) rather than its static description,
  inducing the LLM to make a secondary exfiltration call. **This specifically evades detection
  methods that only scan tool descriptions** — i.e., it defeats the most obvious mitigation
  (description-scanning) by construction.
- CyberArk independently names and elaborates the same "Rug Pull" pattern Invariant Labs
  identified: benign at review, malicious in production.

### 1.3 IEEE S&P 2026 — the first large-scale census: "rife with exploitable gadgets"

*"Parasites in the Toolchain"* (arXiv 2509.06572, accepted IEEE S&P 2026, open-source companion
tool `MCP-SEC`): the first large-scale empirical census of the MCP ecosystem —
**12,230 tools across 1,360 servers.** Finding, quoted: the ecosystem is **"rife with
real-world exploitable gadgets and diverse attack methods."**

This is the finding that upgrades §1.1–1.2 from "demonstrated attack classes exist" to
**"the attack classes are common, not exceptional, across the live ecosystem."** The paper
also names a novel attack class of its own (Parasitic Toolchain Attack) while explicitly
treating tool poisoning and prompt injection as **already-established prior categories** — i.e.
an academic security venue, independent of Invariant Labs and CyberArk, treats §1.1–1.2 as
settled background, not a contested claim.

> **Precision note (2-1 vote):** the paper's self-characterization as the "first" large-scale
> census rests on the authors' own novelty claim, not an independently audited survey of every
> prior study. Treat "first" as the authors' framing.

### 1.4 What is NOT established — read this before treating MCP as "attacked in the wild"

Stated plainly in the run's own caveats, and important for calibrating how Vox talks about this
externally: **no named CVE numbers, and no documented real-world (in-production,
victim-confirmed) MCP breach were surfaced.** Every finding above is a **PoC or research
demonstration** — Invariant Labs, CyberArk, and the academic census — not a confirmed in-the-wild
attack with a CVE identifier. This is a meaningful distinction: the vulnerability classes are
real and systemic (§1.3), but "systemic exploitable gadgets exist" is a different claim than
"users have been breached this way," and only the former is confirmed here.

---

## 2. OpenAI GPT Store — real review process, still 95%+ exploitable (confirmed, high)

### 2.1 The review process is real, not theater

Confirmed 3-0 across multiple sub-claims, sourced to OpenAI's own announcement and its official
**DSA Qualitative Transparency Report** (a primary EU regulatory-compliance document, not
marketing):

- Builders must set sharing to "Everyone" and **verify their Builder Profile** before listing;
  link-only GPTs are excluded from the Store.
- **Combined human + automated review**, plus a user-reporting mechanism, plus **classifiers
  specifically tuned to detect brand-impersonation attempts.**
- Automated policy checks run at **two milestones**: first-made-shareable, and Store submission.
- **Tiered enforcement**: restrict to creator-only access → remove from Store → remove from
  home-page featuring → full account termination for egregious cases.

**This is a materially more thorough review process than the MCP Registry's "minimal-to-no
moderation" (registry doc §1.3).** It is worth naming as the stronger comparator.

### 2.2 And it still doesn't work — the number that matters

Independent peer-reviewed academic research (WPES 2025, ACM-CCS-colocated, DOI-resolved and
arXiv-cross-checked), analyzing **14,904 custom GPTs**:

> **Over 95% lacked adequate security protections.**

Specific exploit rates: **roleplay jailbreaks succeeded 96.51%** of the time; **system-prompt
leakage was exploitable in ~92%** of GPTs; **phishing-content generation was inducible in
91.22%** of GPTs — plus documented malicious-code and data-exfiltration risk.

**This is the single most important comparator for Vox's Phase 3 skill gate.** OpenAI runs
builder verification, combined human+automated review, brand-impersonation classifiers, and a
two-milestone check — a *more* thorough process than anything else surveyed in this research
set — and the resulting marketplace is still exploitable in the overwhelming majority of
listings. **The conclusion is not "review harder." It is "review cannot be the only control,
and post-review runtime behavior needs its own defense."**

---

## 3. Package registry prior art — concrete, and directly transplantable

### 3.1 crates.io — first-come-first-served, explicit anti-squatting rule (confirmed 3-0)

Confirmed verbatim via direct fetch of the governing Rust project RFC:

- **First-come-first-served naming.**
- **Explicit prohibition on name-squatting** — reserving a name without genuine
  functionality/development activity — **enforceable by deletion, sometimes without prior
  notice.**
- **Separately bans buying, selling, or trading package names** for money or compensation.

This is a clean, cheap rule Vox can adopt near-verbatim for skill namespace policy: a name
reserved but never populated with a working skill is deletable on sight, and namespace cannot be
monetized as a separate asset from the skill itself.

### 3.2 npm — Sigstore provenance, GA'd, SLSA-compliant (confirmed 3-0)

- **General Availability: September 26, 2023** (after an April 2023 public beta with 3,800+
  adopting projects) — independently corroborated by GitHub's own changelog.
- Built into the npm CLI via two libraries: **`sigstore-js`** (signing/verification) and
  **`tuf-js`** (secure communication with Sigstore's trust root).
- **Designed to be SLSA-compliant** — gives consumers a **verifiable link from a published
  package to its source code and build instructions**, to detect tampering.

**This is the concrete mechanism that would have defeated the "sleeper rug pull" in §1.1.** A
rug pull works because what runs is not cryptographically tied to what was reviewed. Sigstore
provenance closes exactly that gap: a consumer (or Vox's own install flow) can verify that the
skill body being installed today is the one built from the source that was reviewed, not a
silently-swapped second load.

### 3.3 What's still open — PyPI, and real-world incident postmortems

**Explicitly not established in this run**, stated in its own caveats:

- **No PyPI-specific claims survived verification**, despite being in scope. PyPI's own
  Trusted Publishers / Sigstore integration — a close analogue to npm's — remains unverified
  here and is a natural quick follow-up given npm's mechanism is already confirmed.
- **No documented postmortems of real-world (non-PoC) squatting or supply-chain compromise**
  on npm, crates.io, or PyPI with confirmed downstream victims and assigned CVEs were found in
  this pass.

---

## 4. Corrections during this run

One claim was explicitly refuted during verification and is worth naming as a positive example
of the verification process working: a Wikipedia-sourced characterization of GPT Store review
was **rejected (1-2 vote)** because Wikipedia did not actually detail a specific pre-listing
review process. **The stronger, verified claims in §2.1 instead come from OpenAI's own site and
the DSA transparency PDF** — a case where a plausible secondary source was correctly discarded in
favor of primary documents that took more effort to fetch and read.

Also worth flagging as a pipeline-quality note, not a content correction: a WebFetch
summarization sub-model produced **false negatives** on the OpenAI DSA PDF twice during
verification (reporting content as absent when it was present), resolved only by manually
reading the fetched PDF text. This is recorded because it suggests other automated-summarization
steps in this research pipeline could carry similar undetected miss risk — a reason to trust
high-vote-count, multiply-corroborated findings (like most of §1–3) more than any single-pass
extraction.

---

## 5. Concrete addition to Vox's Phase 3 skill promotion gate

This research adds one gate the induction doc's original 7-step pipeline (induction doc §4)
did not have, because none of the five academic induction systems it drew from were designed
against an adversarial marketplace:

```
8. PROVENANCE BINDING & RE-VERIFICATION ON CHANGE   (new — from this research)
   - Sign the promoted skill body (Sigstore-style: source → build → published artifact,
     §3.2). Vox's install flow verifies the signature before executing, not just before
     displaying the skill's description.
   - Any change to a skill's body AFTER promotion re-enters the full gate from step 1 —
     do not treat "already confirmed" as "confirmed forever." This is the direct fix for
     the sleeper-rug-pull pattern (§1.1): a skill that changes behavior after promotion
     must lose confirmed status until re-verified, not silently keep running with its
     original trust level.
   - Descriptions are reviewed at promotion time; RUNTIME OUTPUT is not, and ATPA (§1.2)
     shows that's exploitable. Where feasible, sandbox skill execution (Vox already has
     this machinery — vox-skill-runtime's WASM/container tiers) so a compromised skill's
     blast radius is contained even if its description passed review.
```

**The GPT Store's 95%+ number (§2.2) is the argument for why step 8 cannot be optional.** A
review-only gate, however thorough, has an empirically demonstrated near-total failure rate
against a determined adversary. Vox's advantage — and the reason this is worth building rather
than accepting the industry's status quo — is that Vox controls the full stack (registry doc §4,
item 2): it can sandbox execution and bind provenance in ways a decentralized, multi-vendor
ecosystem like MCP or the GPT Store cannot.

---

## 6. Open questions

1. **PyPI's own anti-squatting and provenance mechanisms** — unresearched in this pass, natural
   follow-up given npm's mechanism is now well-confirmed.
2. **Real-world (non-PoC) MCP or package-registry incidents with confirmed victims and CVEs** —
   not found in this pass; may not yet exist given how young MCP is (protocol released Nov 2024),
   or may exist and require a differently-scoped search.
3. **OpenAI's own review-effectiveness metrics** (rejection rate, detection time) to compare
   directly against the WPES 2025 academic 95%+ figure — not published, so the gap between
   OpenAI's stated process and its measured outcome cannot be further decomposed with public
   data.

---

## 7. Note on run reliability

4 of 106 agent calls errored on transient upstream rate-limiting (not content/verification
failures) and were not retried within this run; 102 completed. This did not affect the confirmed
findings above, all of which reached full 3-vote consensus independent of the 4 errored calls.
