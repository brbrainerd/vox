# Design: voxlang.org Hosting + Docs Category Overhaul

**Date:** 2026-05-23  
**Status:** Approved  
**Scope:** Two independent sub-projects — A (Cloudflare Pages deployment + DNS) and B (docs sidebar fix)

---

## Context

The Vox documentation site is an Astro 6 + Starlight app in `docs-astro/`, with 749 source `.md` files in `docs/src/`. It currently deploys to GitHub Pages with custom domain `vox-lang.org` (dash form), proxied through Cloudflare — **the site is live today at https://vox-lang.org/**. The desired primary domain is `voxlang.org` (no dash); `vox-lang.org` becomes a 301 redirect alias. Both domains are registered in Namecheap and already managed by Cloudflare nameservers — no Namecheap changes are required.

A frontmatter audit revealed a second critical issue: 640 of 749 docs files use slug-style `category` values (`architecture`, `how-to`, `api-crate`) that do not match the display-string SSOT labels (`Architecture SSOTs`, `How-To Guides`, `API Reference — Crates`). Only the 197 files with `category: "reference"` resolve to a real sidebar section. All other files fall into unlabeled collapsed catch-alls. The sidebar is effectively broken for the majority of content.

Note: 296 of those 749 files live in `docs/src/archive/`, which is already excluded from the sidebar by `EXCLUDED_DIRS` in `sidebar.mjs`. The reclassification script must skip the archive directory entirely.

---

## Sub-project A: Cloudflare Pages Deployment + DNS

### A1 — Wrangler authentication

Run once interactively:
```
wrangler login
```
Opens browser OAuth against the Cloudflare account. Stores token locally.

### A2 — Create CF Pages project

```
cd docs-astro
wrangler pages project create vox-docs --production-branch main
```

Project slug: `vox-docs`. Preview URL: `vox-docs.pages.dev`.

### A3 — GitHub Actions: replace GitHub Pages deploy with wrangler

File: `.github/workflows/docs-deploy.yml`

**Remove:**
- `permissions: pages: write, id-token: write`
- `actions/upload-pages-artifact@v5`
- `actions/deploy-pages@v5`
- `environment: github-pages`

**Add after the Astro build step:**
```yaml
- name: Deploy to Cloudflare Pages
  run: npx wrangler pages deploy dist --project-name=vox-docs --commit-dirty=true
  env:
    CLOUDFLARE_API_TOKEN: ${{ secrets.CF_API_TOKEN }}
    CLOUDFLARE_ACCOUNT_ID: ${{ secrets.CF_ACCOUNT_ID }}
```

**GitHub repo secrets required:**
- `CF_API_TOKEN` — Cloudflare API token with scope `Cloudflare Pages: Edit` (create at dash.cloudflare.com → My Profile → API Tokens → Create Token → "Edit Cloudflare Workers" template, scoped to Pages)
- `CF_ACCOUNT_ID` — found at dash.cloudflare.com → right-hand sidebar on any zone page

All existing quality-gate steps (vox-doc-pipeline Rust binary, doctest-md, check-links, retired-symbol-check, Biome, markdownlint) are unchanged.

### A4 — Custom domains for voxlang.org

`wrangler pages` in v4.56 does **not** expose a `domain add` subcommand. Custom domain attachment must use the Cloudflare REST API. After the first successful deploy:

```bash
CF_API_TOKEN="<token from A3>"
CF_ACCOUNT_ID="<account id from A3>"

curl -X POST \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"voxlang.org"}'

curl -X POST \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"www.voxlang.org"}'
```

Equivalent dashboard path (if preferred manual): **Pages → vox-docs → Custom domains → Set up a custom domain**.

After domain attachment, CF Pages auto-creates the necessary DNS records in the `voxlang.org` Cloudflare zone (`CNAME voxlang.org → vox-docs.pages.dev`, proxied) and provisions TLS automatically via CF SSL for SaaS. No manual DNS edits needed for voxlang.org.

### A5 — vox-lang.org → voxlang.org redirect (301, preserve path)

`vox-lang.org` becomes a redirect-only zone — no content served.

**Cleanup first:** the zone currently has a `CNAME @ → vox-foundation.github.io` record (current GitHub Pages target). Delete this and the GitHub Pages CNAME file in the repo as part of the cutover (see C1 ordering).

**DNS records to add in the `vox-lang.org` Cloudflare zone** (both proxied):
```
A   @    192.0.2.1   (proxied)
A   www  192.0.2.1   (proxied)
```
The 192.0.2.1 address is RFC 5737 documentation space. CF intercepts at edge before any packet is forwarded — the IP is never actually contacted.

**Cloudflare Redirect Rule** (zone-scoped, in dash: **vox-lang.org zone → Rules → Redirect Rules → Create rule**):

- Rule name: `Redirect vox-lang.org → voxlang.org`
- When incoming requests match: `(http.host eq "vox-lang.org") or (http.host eq "www.vox-lang.org")`
- Then: **Dynamic** redirect to
  - Expression: `concat("https://voxlang.org", http.request.uri.path)`
  - Status code: 301
  - Preserve query string: ✓

Redirect Rules are zone-scoped (simpler than account-level Bulk Redirects with named lists) and the right tool for one-zone-to-another canonicalization.

### A6 — Update astro.config.mjs canonical URL

```diff
-  site: 'https://vox-lang.org/',
+  site: 'https://voxlang.org/',
```

Fixes: canonical `<link>` tags, sitemap URLs, Starlight edit-on-GitHub links.

---

## Sub-project B: Docs Category Overhaul

### B1 — New canonical taxonomy

The frontmatter `category` value is set to the exact sidebar display label (eliminates slug↔label mismatch). Categories and their source mappings:

| New `category` value (= sidebar label) | Collapsed | Source path prefix(es) |
|---|---|---|
| `Getting Started` | no | `journeys/`, `getting-started` slug |
| `Tutorials` | no | `tutorials/` |
| `How-To Guides` | no | `how-to/` |
| `Language Reference` | no | `reference/` |
| `API Reference — Crates` | no | `api/` |
| `Examples` | no | `examples/` |
| `Concepts` | no | `explanation/` |
| `Architecture Decisions (ADRs)` | yes | `adr/` |
| `Architecture SSOTs` | yes | `architecture/` |
| `Contributors` | yes | `contributors/` |
| `CI & Quality` | yes | `ci/` |
| `Operations` | yes | `operations/` |

Sections removed from SSOT: "Journeys" (merged → Getting Started), "Reference" catch-all (split), "Explanations" (renamed → Concepts), "API Reference — Keywords", "API Reference — Decorators" (no files use these).

### B2 — Reclassification script

File: `scripts/fix-doc-categories.vox`

Implements path-prefix dispatch. Runs with a `--dry-run` flag that prints a unified diff without writing. Applies atomically otherwise (write to temp, rename). Reports: files changed, files unchanged, files that need manual triage (no matching rule).

Path-prefix rules (applied in order, first match wins):

```
docs/src/archive/**       → SKIP (excluded from sidebar by sidebar.mjs)
docs/src/.well-known/**   → SKIP (excluded from sidebar)
docs/src/assets/**        → SKIP (no markdown anyway)
docs/src/adr/**           → "Architecture Decisions (ADRs)"
docs/src/architecture/**  → "Architecture SSOTs"
docs/src/contributors/**  → "Contributors"
docs/src/ci/**            → "CI & Quality"
docs/src/operations/**    → "Operations"
docs/src/tutorials/**     → "Tutorials"
docs/src/how-to/**        → "How-To Guides"
docs/src/explanation/**   → "Concepts"
docs/src/reference/**     → "Language Reference"
docs/src/ref/**           → "Language Reference"   (alias for reference/, 2 files)
docs/src/api/**           → "API Reference — Crates"
docs/src/journeys/**      → "Getting Started"
docs/src/examples/**      → "Examples"
docs/src/case-studies/**  → "Examples"   (1 file — folded into Examples section)
```

**Root-level files** (no subdir, hand-mapped explicitly in the script):
- `docs/src/AGENTS.md` → `"Architecture SSOTs"` (documentation rules SSOT)
- `docs/src/ref-cli.md` → `"Language Reference"` (legacy redirect page)

The 11 files with no `category` value and any files with no matching path rule are listed in the script output as "needs manual triage." They are small enough to hand-edit after the script runs.

**Counts after rules apply** (excluding archive):
- Architecture SSOTs: ~206 (docs/src/architecture/**)
- Architecture Decisions (ADRs): ~45
- Language Reference: ~101 + 2 + 1 ref-cli = 104
- How-To Guides: ~36
- Concepts: ~16
- CI & Quality: ~15
- Contributors: ~11
- Tutorials: ~6
- Operations: ~5
- Getting Started: ~4 (from journeys/)
- API Reference — Crates: ~2 (+ any api-crate files in subdirs)
- Examples + case-studies: ~2

Roughly 450 in-sidebar files reclassified. The 296 archive files are intentionally skipped.

### B3 — SSOT contract update

File: `contracts/documentation/docs-sidebar-section-order.v1.json`

Rewritten with new section list. `collapsed_sections` array updated to match the table in B1.

### B4 — Validation gates

Run in order:
1. `scripts/fix-doc-categories.vox --dry-run` → review diff
2. `scripts/fix-doc-categories.vox` → apply
3. `vox ci check-docs-ssot` → regenerate `docs/agents/doc-inventory.json`
4. `cd docs-astro && pnpm build` → Starlight build (catches broken frontmatter)
5. Internal link checker passes
6. No new lint errors

---

## Sub-project C: Live Site Verification (Playwright)

### C1 — Check current state first

The current live site is `https://vox-lang.org/` served via GitHub Pages with Cloudflare proxy (`x-github-request-id` header confirms origin). `voxlang.org` is unconfigured.

Before any DNS or workflow change, capture baseline:
1. Playwright snapshot of `https://vox-lang.org/` (home, sidebar render, search)
2. Save sidebar HTML to compare post-overhaul (should show many more functional sections)
3. Record current Cloudflare DNS records in both zones via `wrangler` or dashboard export
4. Confirm `vox-lang.org` zone has `CNAME @ → vox-foundation.github.io` (to be removed in A5)

**Cutover order to avoid breakage:**
1. Deploy to CF Pages (vox-docs.pages.dev works in parallel with current site)
2. Add custom domains to CF Pages project (TLS provisions)
3. In `voxlang.org` zone: CF Pages adds the CNAME automatically
4. In `vox-lang.org` zone: replace the old `CNAME @ → vox-foundation.github.io` with the proxied A record + Redirect Rule (this is the moment of cutover for vox-lang.org traffic)
5. Verify with smoke tests
6. Remove GH Pages workflow steps and the `docs-astro/public/CNAME` file (if present) only after CF Pages is confirmed serving

### C2 — Smoke test file

File: `docs-astro/tests/smoke.spec.ts`

Test suite that runs after deployment confirms live:

| Check | Expected |
|---|---|
| `GET https://voxlang.org` | 200, `<title>` contains "Vox" |
| `GET https://vox-lang.org` | 301 redirect chain → `https://voxlang.org` |
| `GET https://www.voxlang.org` | 200 |
| Sidebar contains "Getting Started" | true |
| Sidebar contains "How-To Guides" | true |
| Sidebar contains "Tutorials" | true |
| Search input present (pagefind) | true |
| `GET https://voxlang.org/<first getting-started page>` | 200 |

### C3 — CI integration

Add as final step in `docs-deploy.yml` after wrangler deploy:
```yaml
- name: Smoke test live site
  run: cd docs-astro && npx playwright install chromium --with-deps && npx playwright test tests/smoke.spec.ts
  env:
    BASE_URL: https://voxlang.org
```

If the CI environment cannot reach the public internet, the smoke test is run locally via `npx playwright test` and the result reported manually.

---

## Execution order

1. **C1 (baseline):** Snapshot current `vox-lang.org` live state; record current DNS in both zones
2. **B1–B4:** Run reclassification script (dry-run, review, apply), update SSOT, build locally to validate
3. **A1–A2:** Authenticate wrangler, create CF Pages project (local, interactive)
4. **A6:** Update `astro.config.mjs` site URL to `https://voxlang.org/`
5. **A3:** Update `docs-deploy.yml` (add CF deploy step, keep GH Pages steps temporarily — see below)
6. **First deploy:** Push a branch, manually run `wrangler pages deploy dist` from local build to verify `vox-docs.pages.dev` works
7. **A4:** Attach custom domains via CF REST API (or dashboard) — `voxlang.org`, `www.voxlang.org` go live on CF Pages
8. **Add `vox-lang.org` to CF Pages too**, briefly, so both old + new domains serve from CF Pages (zero downtime during DNS swap)
9. **A5:** In `vox-lang.org` zone, swap from "CNAME @ → vox-foundation.github.io" to redirect setup (proxied A record + Redirect Rule); remove `vox-lang.org` from CF Pages domains list
10. **C2–C3:** Run smoke test against live site
11. Merge to main → CI deploys via wrangler; smoke test runs
12. **Cleanup:** remove GitHub Pages workflow steps and the two CNAME files (`./CNAME` and `docs-astro/public/CNAME`, both currently containing `vox-lang.org`) after a few days of stable CF Pages operation

---

## Out of scope

- Content authoring / updating individual doc files
- `docs/src/archive/` contents (excluded from sidebar by `sidebar.mjs`)
- `docs/src/.well-known/` (excluded from sidebar)
- Cloudflare Analytics or Web Analytics setup
- Workers KV / R2 / D1 (not needed for static Astro)
