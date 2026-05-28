# SCIENTIA Phase G — Vox-Native Publication Reading Surface

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** outline.

**Goal:** Provide a canonical web landing page for each published manifest, hosted on Vox-owned infrastructure, distinct from Zenodo/arXiv deposits. Distill-style web-native reading experience; Living-Review version-history surface; embedded nanopub viewer; reply-thread inline; Highwire-style meta tags for Google Scholar pickup.

**Architecture:** Add a `findings/<trusty-uri>` route to the docs SSG (the existing site at `docs/src/`). The route is generated at build time from `publication_manifests` rows that have at least one `succeeded` scholarly submission with non-private visibility. Each manifest's RO-Crate text body is rendered as markdown with verified-claim-table sidebar, version-history block, reply-thread block, and Highwire meta tags. The dashboard separately gets an *edit* surface (post-publication revisions, reply triage) but the SSG is the canonical *read* surface. The route URL contains the Trusty URI suffix so it's content-addressable.

**Tech Stack:** Whatever the docs SSG already uses (verify in Task G1; from the codebase signals it's likely `mdbook`-style with `vox-doc-pipeline`); existing `vox-ro-crate`; existing `vox-nanopub`. No new external deps if the SSG already renders markdown.

**Strategic context:** [Gap-map §2 Gap G](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-g--vox-native-publication-reading-surface); [Finalization Plan §4 (artifact-graph unit of trust)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#4-artifact-format--the-unit-of-trust-is-not-the-paper).

**Out of scope:**
- Replacing external deposits (Zenodo/arXiv stay canonical for DOI/preprint).
- Comment threading from external users (use existing reply-window machinery; G hosts inline display, not authoring).
- Search across published findings (deferred to Phase H or a later phase).

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Create | `crates/vox-findings-site/Cargo.toml` | L3 site-builder crate |
| Create | `crates/vox-findings-site/src/lib.rs` | Build entry: read manifests → emit routes |
| Create | `crates/vox-findings-site/src/render.rs` | Markdown + version-history + claim-table renderer |
| Create | `crates/vox-findings-site/src/meta_tags.rs` | Highwire meta tag emitter |
| Create | `crates/vox-findings-site/src/nanopub_viewer.rs` | Embedded nanopub viewer (renders TriG → readable claim) |
| Create | `crates/vox-findings-site/templates/finding.html.tera` | Per-finding page template |
| Create | `crates/vox-findings-site/templates/index.html.tera` | Findings index page |
| Modify | `docs/src/` SUMMARY auto-generation surface (verify path; do **NOT** hand-edit `SUMMARY.md`) | Regenerate to include `findings/` tree |
| Modify | `crates/vox-doc-pipeline/src/<entry>` | Hook the findings-site builder into doc-pipeline regeneration |
| Modify | `docs/src/architecture/where-things-live.md` | Add row: "Findings reading surface" |
| Modify | `docs/src/architecture/layers.toml` | Register at L3 (site-builder) |

LoC budget: ~1000 LoC + ~300 tests + template files.

---

## Tasks (headings only)

### Task G1: Identify the SSG
Verify whether the docs site is mdbook, mdx, custom, etc. Mirror its conventions for route emission, asset handling, and template engine.

### Task G2: Crate scaffold + manifest reader
Read manifests filtered by:
- `state` ∈ {`published`, `living_review_active`}.
- `visibility` ≠ `private`.
- ≥1 `succeeded` row in `scholarly_submissions`.

### Task G3: Per-finding page render
Sections:
- Title + author block.
- Abstract.
- Body (markdown from RO-Crate `text/main.md`).
- Verified-claims table (one row per atomic claim with Trusty URI → embedded nanopub viewer).
- Version history block (canonical URL = "latest"; per-version DOIs listed).
- Reply thread (inline, from `publication_status_events` with code `reply_received`).
- Retraction banner (if applicable, prominent).
- AI-disclosure block.
- Competing-interests block.
- CRediT roles.
- Citation footer with how-to-cite.

### Task G4: Highwire meta tags
`citation_title`, `citation_author`, `citation_publication_date`, `citation_doi`, `citation_pdf_url`, `citation_abstract_html_url`. Google Scholar pickup.

### Task G5: Embedded nanopub viewer
Renders TriG → readable claim with provenance pop-over. Pure HTML/JS; no external service call.

### Task G6: Findings index page
List of all published findings, filterable by `candidate_class` and date.

### Task G7: Build integration
Hook into `vox-doc-pipeline` regeneration. **Do not** hand-edit `SUMMARY.md` or `architecture-index.md` (auto-generated per project rule).

### Task G8: Living-Review canonical URL
`/findings/<canonical-slug>/latest` redirects to the most recent version. Each version has its own permanent URL.

### Task G9: Retraction handling
If the manifest has a retraction nanopub, the page top renders a prominent banner; the original content remains accessible below (per COPE practice).

### Task G10: Tests
- Render-snapshot tests for the three primary template layouts.
- Highwire meta tag presence assertion.
- Retraction-banner presence on retracted manifest fixture.

### Task G11: Documentation
- How-to: "Where my published finding lives on the open web".
- Architecture entry in where-things-live.

---

## Acceptance criteria

1. `cargo run -p vox-doc-pipeline` regenerates the findings tree without manual SUMMARY edits.
2. Per-finding page renders all sections including embedded nanopub viewer.
3. Highwire meta tags validate against Google Scholar inclusion checks (manual verification step).
4. Version history shows distinct DOIs for distinct versions; canonical URL points to latest.
5. Retraction banner renders on retracted fixture.
6. The site builds with no hand-edits to auto-generated files.

---

## Open questions

- **OQ-G1.** Hosting. Same domain as docs (`docs.vox-lang.org/findings/...`) or subdomain (`findings.vox-lang.org`)? Recommendation: subpath of docs for Phase G; subdomain when volume justifies it.
- **OQ-G2.** Dashboard vs SSG split. Recommendation: SSG = read-only public canonical surface. Dashboard = authenticated edit surface for reply triage, retraction issuing, version management. Phase G builds SSG only; dashboard edit surface is Phase H or a follow-up.
- **OQ-G3.** Search indexing. Robots.txt allow Google Scholar but rate-limit aggressive crawlers. Defer to Phase G+1.
- **OQ-G4.** Comment authoring. Out of scope for Phase G. Reply-window authoring already happens via existing CLI/MCP.
- **OQ-G5.** Old-style HTML preview vs JS-rendered. Recommendation: server-rendered HTML for accessibility and citation; sprinkle JS only for nanopub-viewer interactivity.

---

## Dependencies

- **Upstream:** Finalization Phase 4 (RO-Crate) ✅; Phase 3 (living-review version DOIs, retraction nanopubs) ✅.
- **Downstream:** None hard.

---

## Cross-references

- Gap: [gap-map §2 Gap G](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-g--vox-native-publication-reading-surface)
- RO-Crate: `crates/vox-ro-crate/`
- Nanopub: `crates/vox-nanopub/`
- Doc pipeline: `crates/vox-doc-pipeline/`
