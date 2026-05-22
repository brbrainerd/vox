---
title: "scripts/show/ — Raw Nerve automation"
description: "Vox programs that automate the Raw Nerve content pipeline. Drafts only; user publishes."
category: "scripts"
status: experimental
training_eligible: false
---
# `scripts/show/` — Raw Nerve content automation

Vox programs that automate the recurring work of producing **Raw Nerve**, the Vox Foundation's literary engineering journal. Cross-promotion is structural: every artifact the show needs is itself a Vox program shipped in this repo.

**Rule:** automation produces drafts; you publish. No auto-tweet, no auto-YouTube upload, no auto-email. Drafts only — then a human clicks send.

## Files

| Script | Purpose | Stage |
|---|---|---|
| [`script.vox`](script.vox) | Whisper transcript → 3-act outline (Acta/Documenta/Analogia/Mens) | foundational |
| [`topic-suggest.vox`](topic-suggest.vox) | git log + issues + PRs → 3 ranked episode candidates | foundational |
| [`title-workshop.vox`](title-workshop.vox) | outline → 10 ranked title candidates | foundational |

Future additions (built when needed, see [funding-campaign implementation plan](https://github.com/vox-foundation/vox-funding-plan)):

- `cross-post.vox` — single MD → YouTube + X + Reddit + HN + Discord drafts
- `newsletter.vox` — monthly auto-digest from `git log`
- `community-digest.vox` — Friday digest of Discord + GitHub Qs needing human attention
- `thermometer.vox` — OC + GH Sponsors APIs → donation widget (build when donations exist)
- `sponsor-wall.vox` — nightly auto-publish to landing page
- `tile.vox` — new sponsor → PNG tile baked into next intro
- `publish.vox` — vox-scientia publication record per episode
- `shorts.vox` — auto-clip 60-90s shorts from main video

## Conventions

- Every script ≤ 500 LoC (TOESTUB-compliant)
- Every script has a 1-line header comment describing what it does and what it depends on
- Every script accepts `--dry-run` (when applicable) and prints what it would do without doing it
- All LLM calls route through `vox-mens` or the unified provider routing — no hardcoded keys
- All output drafts go under `~/raw-nerve/drafts/` or a `--out` flag
- Drafts: never auto-publish

## Why this exists

The Raw Nerve channel pitches itself as a publication *of* the Vox Foundation — every meta-tool used to produce the show is itself a Vox program in this repo. Watching the show is watching Vox run. New viewers learn the language by encountering it as the substrate of the content they're consuming.

See: [the funding-campaign design doc](https://github.com/vox-foundation/vox-funding-plan) for the strategic frame.
