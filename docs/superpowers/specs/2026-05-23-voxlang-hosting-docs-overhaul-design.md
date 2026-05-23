# Design: voxlang.org Hosting + Docs Category Overhaul

**Date:** 2026-05-23  
**Status:** Approved  
**Scope:** Two independent sub-projects — A (Cloudflare Pages deployment + DNS) and B (docs sidebar fix)

---

## Context

The Vox documentation site is an Astro 6 + Starlight app in `docs-astro/`, with 749 source `.md` files in `docs/src/`. It currently deploys to GitHub Pages. The canonical URL in `astro.config.mjs` is `https://vox-lang.org/` (dash form); the desired primary domain is `voxlang.org` (no dash), with `vox-lang.org` as a 301 redirect alias. Both domains are registered in Namecheap and already managed by Cloudflare nameservers — no Namecheap changes are required.

A frontmatter audit revealed a second critical issue: 640 of 749 docs files use slug-style `category` values (`architecture`, `how-to`, `api-crate`) that do not match the display-string SSOT labels (`Architecture SSOTs`, `How-To Guides`, `API Reference — Crates`). Only the 197 files with `category: "reference"` resolve to a real sidebar section. All other files fall into unlabeled collapsed catch-alls. The sidebar is effectively broken for the majority of content.

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

After first successful deploy:
```
wrangler pages domain add vox-docs voxlang.org
wrangler pages domain add vox-docs www.voxlang.org
```

CF Pages auto-creates DNS records in the `voxlang.org` Cloudflare zone and provisions TLS automatically. No manual DNS edits needed for voxlang.org.

### A5 — vox-lang.org → voxlang.org redirect (301, preserve path)

`vox-lang.org` is a redirect-only zone — it never serves content.

**DNS records in the `vox-lang.org` Cloudflare zone** (both proxied):
```
A   @    192.0.2.1   (proxied)
A   www  192.0.2.1   (proxied)
```
The 192.0.2.1 address is a documentation/discard address (RFC 5737). CF intercepts at edge before any packet is forwarded.

**Cloudflare Bulk Redirect** in the `vox-lang.org` zone:
- Source URL: `vox-lang.org/`  
- Target URL: `https://voxlang.org/`  
- Status: 301  
- Options: Include subdomains ✓, Preserve path ✓, Preserve query string ✓

This covers `vox-lang.org`, `www.vox-lang.org`, and any path under either.

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
docs/src/adr/**           → "Architecture Decisions (ADRs)"
docs/src/architecture/**  → "Architecture SSOTs"
docs/src/contributors/**  → "Contributors"
docs/src/ci/**            → "CI & Quality"
docs/src/operations/**    → "Operations"
docs/src/tutorials/**     → "Tutorials"
docs/src/how-to/**        → "How-To Guides"
docs/src/explanation/**   → "Concepts"
docs/src/reference/**     → "Language Reference"
docs/src/api/**           → "API Reference — Crates"
docs/src/journeys/**      → "Getting Started"
docs/src/examples/**      → "Examples"
```

The 11 files with no `category` and any files with no matching path rule are listed in the script output as "needs manual triage." They are small enough to hand-edit after the script runs.

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

Before touching the deploy workflow, run a Playwright (or curl) check against the current GitHub Pages URL to record what's live today and confirm the baseline.

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

1. **A1–A2:** Authenticate wrangler, create CF Pages project (local, interactive)
2. **A6:** Update `astro.config.mjs` site URL
3. **A3:** Update `docs-deploy.yml` (add CF deploy step, remove GH Pages steps)
4. **A4:** Add custom domains via wrangler
5. **A5:** Configure vox-lang.org Bulk Redirect in Cloudflare dashboard
6. **B1–B4:** Run reclassification script, update SSOT, validate
7. **C1:** Check current GH Pages baseline
8. **C2–C3:** Add smoke test, run it against live site post-deploy
9. Merge to main → CI deploys → smoke test runs

---

## Out of scope

- Content authoring / updating individual doc files
- `docs/src/archive/` contents (excluded from sidebar by `sidebar.mjs`)
- `docs/src/.well-known/` (excluded from sidebar)
- Cloudflare Analytics or Web Analytics setup
- Workers KV / R2 / D1 (not needed for static Astro)
