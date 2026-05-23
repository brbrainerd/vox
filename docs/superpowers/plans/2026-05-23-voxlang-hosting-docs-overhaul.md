# voxlang.org Hosting + Docs Category Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate Vox docs from GitHub Pages → Cloudflare Pages on `voxlang.org` (primary) with `vox-lang.org` as a 301 redirect alias; fix the broken Starlight sidebar by realigning frontmatter `category` values across 449 in-sidebar files.

**Architecture:** Three phases run mostly independently. **Phase B** (docs overhaul) is pure repo work and ships first. **Phase A** (CF Pages) is infra/CI work. **Phase X** is the DNS cutover. **Phase C** wraps a Playwright smoke test around both the baseline and the post-deploy state. Cleanup runs last to remove GitHub Pages artifacts only after CF Pages is confirmed stable.

**Tech Stack:** Astro 6 + Starlight 0.38, Vox `.vox` scripting language (per CLAUDE.md), Cloudflare Pages + wrangler v4.56, Playwright (smoke tests), GitHub Actions (CI).

**Spec:** [`docs/superpowers/specs/2026-05-23-voxlang-hosting-docs-overhaul-design.md`](../specs/2026-05-23-voxlang-hosting-docs-overhaul-design.md)

---

## Phase 0: Baseline Snapshot

Capture current `vox-lang.org` live state before any change so we can verify nothing regresses.

### Task 0.1: Install Playwright in docs-astro

**Files:**
- Modify: `docs-astro/package.json` (add devDependency)
- Modify: `docs-astro/pnpm-lock.yaml` (regenerated)

- [ ] **Step 1: Install Playwright**

Run from repo root:
```bash
cd docs-astro && pnpm add -D @playwright/test && cd ..
```

Expected: `pnpm-lock.yaml` updated, `@playwright/test` appears in devDependencies.

- [ ] **Step 2: Install Chromium browser**

```bash
cd docs-astro && npx playwright install chromium && cd ..
```

Expected: Chromium installed to user-local Playwright cache.

- [ ] **Step 3: Commit**

```bash
git add docs-astro/package.json docs-astro/pnpm-lock.yaml
git commit -m "chore(docs-astro): add Playwright for live-site smoke tests"
```

---

### Task 0.2: Add Playwright config

**Files:**
- Create: `docs-astro/playwright.config.ts`

- [ ] **Step 1: Write `docs-astro/playwright.config.ts`**

```typescript
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: 'list',
  use: {
    baseURL: process.env.BASE_URL ?? 'https://vox-lang.org',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
```

- [ ] **Step 2: Commit**

```bash
git add docs-astro/playwright.config.ts
git commit -m "chore(docs-astro): add Playwright config (baseURL via env)"
```

---

### Task 0.3: Capture baseline of current vox-lang.org

**Files:**
- Create: `docs-astro/tests/baseline.spec.ts`
- Create: `docs-astro/test-results/baseline-snapshot.txt` (output, git-ignored)

- [ ] **Step 1: Write the baseline capture test**

`docs-astro/tests/baseline.spec.ts`:
```typescript
import { test, expect } from '@playwright/test';
import { writeFileSync, mkdirSync } from 'node:fs';

test('baseline: capture current vox-lang.org state', async ({ page, request }) => {
  // Direct response headers
  const headResp = await request.get('https://vox-lang.org/', { maxRedirects: 0 });
  const head = `Status: ${headResp.status()}\nHeaders: ${JSON.stringify(headResp.headers(), null, 2)}`;

  // Page render
  await page.goto('https://vox-lang.org/');
  const title = await page.title();
  const sidebarText = await page.locator('nav, [class*="sidebar" i]').first().innerText().catch(() => '(no sidebar matched)');
  const h1 = await page.locator('h1').first().innerText().catch(() => '(no h1)');

  const snapshot = [
    '=== HEAD ===', head,
    '=== TITLE ===', title,
    '=== H1 ===', h1,
    '=== SIDEBAR (top 60 lines) ===',
    sidebarText.split('\n').slice(0, 60).join('\n'),
  ].join('\n\n');

  mkdirSync('test-results', { recursive: true });
  writeFileSync('test-results/baseline-snapshot.txt', snapshot, 'utf8');

  // Sanity: site is live today
  expect(headResp.status()).toBe(200);
  expect(title.toLowerCase()).toContain('vox');
});
```

- [ ] **Step 2: Run the baseline test**

```bash
cd docs-astro && npx playwright test tests/baseline.spec.ts
```

Expected: PASS. `docs-astro/test-results/baseline-snapshot.txt` written.

- [ ] **Step 3: Inspect the snapshot**

```bash
cat docs-astro/test-results/baseline-snapshot.txt
```

Verify: status 200, title contains "Vox", sidebar text captured. **Note what sections appear** — these are the only ones currently rendering. Most should be the collapsed catch-all sections at the bottom (per the audit finding).

- [ ] **Step 4: Add test-results to .gitignore**

Append to `docs-astro/.gitignore` (create if missing):
```
test-results/
playwright-report/
.playwright/
```

- [ ] **Step 5: Commit**

```bash
git add docs-astro/tests/baseline.spec.ts docs-astro/.gitignore
git commit -m "test(docs-astro): capture baseline of live vox-lang.org for regression compare"
```

---

## Phase B: Docs Category Overhaul

### Task B.1: Write the reclassification `.vox` script

**Files:**
- Create: `scripts/fix-doc-categories.vox`

- [ ] **Step 1: Write the script**

`scripts/fix-doc-categories.vox`:
```vox
import fs
import path
import regex
import env

fn category_for_path(rel: str) -> str {
    // Skip directories that are excluded from the sidebar entirely
    if regex.is_match(rel, r"^archive/") { return "" }
    if regex.is_match(rel, r"^\.well-known/") { return "" }
    if regex.is_match(rel, r"^assets/") { return "" }

    // Path-prefix rules (first match wins)
    if regex.is_match(rel, r"^adr/")          { return "Architecture Decisions (ADRs)" }
    if regex.is_match(rel, r"^architecture/") { return "Architecture SSOTs" }
    if regex.is_match(rel, r"^contributors/") { return "Contributors" }
    if regex.is_match(rel, r"^ci/")           { return "CI & Quality" }
    if regex.is_match(rel, r"^operations/")   { return "Operations" }
    if regex.is_match(rel, r"^tutorials/")    { return "Tutorials" }
    if regex.is_match(rel, r"^how-to/")       { return "How-To Guides" }
    if regex.is_match(rel, r"^explanation/")  { return "Concepts" }
    if regex.is_match(rel, r"^reference/")    { return "Language Reference" }
    if regex.is_match(rel, r"^ref/")          { return "Language Reference" }
    if regex.is_match(rel, r"^api/")          { return "API Reference — Crates" }
    if regex.is_match(rel, r"^journeys/")     { return "Getting Started" }
    if regex.is_match(rel, r"^examples/")     { return "Examples" }
    if regex.is_match(rel, r"^case-studies/") { return "Examples" }

    // Root-level files (handled explicitly)
    if rel == "AGENTS.md"  { return "Architecture SSOTs" }
    if rel == "ref-cli.md" { return "Language Reference" }

    return "UNMAPPED"
}

fn update_category_line(content: str, new_cat: str) -> str {
    // Existing category line: replace it
    let pattern = r"(?m)^category:\s*.*$"
    let replacement = "category: \"" + new_cat + "\""

    if regex.is_match(content, pattern) {
        return regex.replace(content, pattern, replacement)
    }

    // No category line: insert after opening ---
    let lines = content.split("\n")
    if lines.len() < 2 || lines[0] != "---" {
        return content  // no frontmatter — leave alone
    }
    let mut close_idx = 1
    while close_idx < lines.len() && lines[close_idx] != "---" {
        close_idx = close_idx + 1
    }
    if close_idx >= lines.len() {
        return content  // unterminated frontmatter
    }
    lines.insert(close_idx, replacement)
    return lines.join("\n")
}

fn main() {
    let dry_run = env.get("DRY_RUN") == "1"
    let docs_dir = "docs/src"
    let files = fs.list_recursive(docs_dir, "*.md")

    let mut changed = 0
    let mut skipped = 0
    let mut already_correct = 0
    let mut unmapped: list<str> = []

    for f in files {
        let rel = path.relative(docs_dir, f).replace("\\", "/")
        let new_cat = category_for_path(rel)

        if new_cat == "" {
            skipped = skipped + 1
            continue
        }
        if new_cat == "UNMAPPED" {
            unmapped.push(rel)
            continue
        }

        let content = fs.read_to_string(f)
        let new_content = update_category_line(content, new_cat)

        if content == new_content {
            already_correct = already_correct + 1
            continue
        }

        if dry_run {
            print("WOULD UPDATE: " + rel + " -> " + new_cat)
        } else {
            fs.write_to_file(f, new_content)
            print("UPDATED: " + rel + " -> " + new_cat)
        }
        changed = changed + 1
    }

    print("---")
    print("Changed:         " + changed.to_string())
    print("Already correct: " + already_correct.to_string())
    print("Skipped (excluded dirs): " + skipped.to_string())
    print("Unmapped (needs manual triage): " + unmapped.len().to_string())
    for u in unmapped {
        print("  " + u)
    }
    if dry_run {
        print("(dry-run — no files written. Re-run without DRY_RUN=1 to apply.)")
    }
}
```

> **Vox syntax note:** if `regex.is_match`, `lines.insert`, `to_string`, or `list<str>` don't match the local Vox stdlib exactly, consult `scripts/migrate-arrows.vox` and `scripts/migrate-corpus.vox` for tested patterns and adapt. Functionality must stay identical.

- [ ] **Step 2: Commit (script only — no docs touched yet)**

```bash
git add scripts/fix-doc-categories.vox
git commit -m "feat(scripts): add fix-doc-categories.vox to realign frontmatter with SSOT labels"
```

---

### Task B.2: Dry-run and review

- [ ] **Step 1: Run the script with `DRY_RUN=1`**

```bash
DRY_RUN=1 vox run scripts/fix-doc-categories.vox > /tmp/recat-dryrun.log 2>&1
```

Expected: completes without error. Last line confirms dry-run.

- [ ] **Step 2: Review the summary**

```bash
tail -30 /tmp/recat-dryrun.log
```

Expected counts (approximate, from spec):
- Changed: ~445–460
- Already correct: small (only files whose path-prefix already matches the new label)
- Skipped: ~296 (archive) + 1 (.well-known) + small (assets/)
- Unmapped: ≤ 3 (root-level files should already be handled — anything else needs a new rule)

- [ ] **Step 3: Spot-check 10 random changes**

```bash
grep "WOULD UPDATE" /tmp/recat-dryrun.log | shuf -n 10
```

Verify each mapping is sensible. For any file that surprises you, open it and check.

- [ ] **Step 4: Triage any unmapped files**

```bash
grep "^  " /tmp/recat-dryrun.log | tail -20
```

For each unmapped file: either add a path rule to the script and re-run dry-run, or note that it'll be hand-edited in Task B.3.

If the script changed:
```bash
git add scripts/fix-doc-categories.vox
git commit -m "feat(scripts): extend fix-doc-categories.vox path rules"
```

---

### Task B.3: Apply reclassification

**Files:**
- Modify: ~450 files under `docs/src/` (frontmatter `category` line only)

- [ ] **Step 1: Run the script for real**

```bash
vox run scripts/fix-doc-categories.vox > /tmp/recat-apply.log 2>&1
```

Expected: same change count as dry-run; no "WOULD UPDATE" lines (now "UPDATED").

- [ ] **Step 2: Verify with frontmatter audit**

```bash
pwsh -Command "
\$files = Get-ChildItem -Path 'docs\src' -Recurse -Filter '*.md'
\$cats = @{}
foreach (\$f in \$files) {
    \$content = Get-Content \$f.FullName -Raw
    if (\$content -match '(?m)^category:\s*[\x22\x27]?([^\x22\x27\n]+?)[\x22\x27]?\s*\$') {
        \$cat = \$Matches[1].Trim()
        if (-not \$cats.ContainsKey(\$cat)) { \$cats[\$cat] = 0 }
        \$cats[\$cat]++
    }
}
\$cats.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object { \"\$(\$_.Value.ToString().PadLeft(4))  \$(\$_.Key)\" }
"
```

Expected: top categories are now the new label strings (`Architecture SSOTs`, `Language Reference`, `How-To Guides`, etc.). No more `architecture`, `how-to`, `api-crate` slug values (except in `archive/` which we skipped — those don't show up if our audit filters archive).

- [ ] **Step 3: Hand-edit any leftover unmapped files**

For each file from the dry-run "Unmapped" list, open it and set the correct category by hand.

- [ ] **Step 4: Commit**

```bash
git add docs/src
git commit -m "docs: realign frontmatter category values with sidebar SSOT labels

Run scripts/fix-doc-categories.vox to map slug-style category values
(architecture, how-to, api-crate) to the display-string labels used by
contracts/documentation/docs-sidebar-section-order.v1.json. Fixes the
broken sidebar where 640 of 749 files were falling into unlabeled
collapsed catch-all sections."
```

---

### Task B.4: Update the sidebar SSOT contract

**Files:**
- Modify: `contracts/documentation/docs-sidebar-section-order.v1.json`

- [ ] **Step 1: Rewrite the SSOT**

```json
{
  "x-vox-version": 1,
  "sections": [
    "Getting Started",
    "Tutorials",
    "How-To Guides",
    "Language Reference",
    "API Reference — Crates",
    "Examples",
    "Concepts",
    "Architecture Decisions (ADRs)",
    "Architecture SSOTs",
    "Contributors",
    "CI & Quality",
    "Operations"
  ],
  "collapsed_sections": [
    "Architecture Decisions (ADRs)",
    "Architecture SSOTs",
    "Contributors",
    "CI & Quality",
    "Operations"
  ]
}
```

- [ ] **Step 2: Build locally to validate**

```bash
cd docs-astro && pnpm install --frozen-lockfile && pnpm build
```

Expected: build succeeds. Look for warnings about unknown categories — there should be none. Pages count should match the in-sidebar file count (~450 + the ~110 already-correct files = ~560).

- [ ] **Step 3: Visual spot-check the local sidebar**

```bash
cd docs-astro && pnpm preview
```

Open the local preview URL (printed by `pnpm preview`). Check the sidebar:
- Top-level sections are: Getting Started, Tutorials, How-To Guides, Language Reference, API Reference — Crates, Examples, Concepts (expanded), then collapsed Architecture Decisions (ADRs), Architecture SSOTs, Contributors, CI & Quality, Operations.
- Click a few sections — pages should appear under them.
- No empty sections; no unlabeled collapsed catch-alls at the bottom.

Stop preview with Ctrl-C.

- [ ] **Step 4: Commit**

```bash
git add contracts/documentation/docs-sidebar-section-order.v1.json
git commit -m "docs(ssot): refresh sidebar section order — drop Journeys/Reference/Explanations catch-alls"
```

---

### Task B.5: Regenerate dependent artifacts

**Files:**
- Modify: `docs/agents/doc-inventory.json` (regenerated by `vox ci check-docs-ssot`)
- Possibly others auto-generated (per CLAUDE.md memory note about auto-generated files)

- [ ] **Step 1: Regenerate doc inventory**

```bash
vox ci check-docs-ssot
```

Expected: `docs/agents/doc-inventory.json` updated to reflect new categories.

- [ ] **Step 2: Run the full docs quality pipeline locally**

```bash
cargo run -p vox-doc-pipeline
```

Expected: passes with no errors.

```bash
cd docs-astro && pnpm build
```

Expected: passes.

- [ ] **Step 3: Commit regenerated artifacts**

```bash
git add docs/agents/doc-inventory.json
# Add any other auto-generated files the pipeline touched
git status
# review what changed; stage any other regen artifacts that appear
git commit -m "docs(agents): regenerate doc-inventory.json after category overhaul"
```

---

## Phase A: Cloudflare Pages Setup

### Task A.1: Wrangler login (interactive)

> This step requires user interaction in a browser. The agent should pause and prompt the human operator.

- [ ] **Step 1: Run wrangler login**

```bash
wrangler login
```

Expected: browser opens to Cloudflare OAuth. User clicks "Allow." Terminal prints `Successfully logged in.`

- [ ] **Step 2: Verify**

```bash
wrangler whoami
```

Expected: account email and account ID printed.

- [ ] **Step 3: Record the Account ID** — copy from the `whoami` output. You'll need it for Task A.3.

---

### Task A.2: Create the Cloudflare Pages project

- [ ] **Step 1: Create the project**

```bash
cd docs-astro && wrangler pages project create vox-docs --production-branch main
```

Expected: confirmation that project `vox-docs` was created. Output mentions `vox-docs.pages.dev`.

- [ ] **Step 2: Verify**

```bash
wrangler pages project list
```

Expected: `vox-docs` appears in the list.

---

### Task A.3: Create CF API token + add GitHub secrets (interactive)

> This step requires the Cloudflare dashboard and GitHub repo Settings. The agent should pause and provide the operator with the exact clicks/values.

- [ ] **Step 1: Create the API token**

In Cloudflare dashboard:
1. Profile menu → **My Profile** → **API Tokens** → **Create Token**
2. Select **Create Custom Token**
3. Token name: `github-actions-vox-docs-deploy`
4. Permissions:
   - **Account** → **Cloudflare Pages** → **Edit**
5. Account Resources: **Include — Specific account — `<your account>`**
6. TTL: (leave default, or set 1 year)
7. **Continue to summary** → **Create Token**
8. **Copy the token now** (only shown once)

- [ ] **Step 2: Add GitHub repo secrets**

In GitHub repo `vox-foundation/vox`:
1. **Settings** → **Secrets and variables** → **Actions** → **New repository secret**
2. Add `CF_API_TOKEN` with the token value from Step 1
3. Add another secret `CF_ACCOUNT_ID` with the Account ID from Task A.1.Step 3

Verify by checking the secrets list shows both names (values are hidden).

---

### Task A.4: Update astro.config.mjs canonical URL

**Files:**
- Modify: `docs-astro/astro.config.mjs:10`

- [ ] **Step 1: Edit the file**

Change line 10:
```diff
-  site: 'https://vox-lang.org/',
+  site: 'https://voxlang.org/',
```

- [ ] **Step 2: Build to verify**

```bash
cd docs-astro && pnpm build
```

Expected: build succeeds. Check `dist/sitemap-0.xml` to confirm URLs are now `https://voxlang.org/...`.

- [ ] **Step 3: Commit**

```bash
git add docs-astro/astro.config.mjs
git commit -m "docs(astro): set canonical site URL to https://voxlang.org/"
```

---

### Task A.5: Update docs-deploy.yml — add CF Pages deploy

**Files:**
- Modify: `.github/workflows/docs-deploy.yml`

**Approach:** add a new `deploy-cloudflare` job that runs *in parallel* with the existing `deploy-pages` job. We keep GH Pages temporarily so we have a fallback during cutover; remove it in Phase Z after CF Pages is stable.

- [ ] **Step 1: Edit `.github/workflows/docs-deploy.yml`**

Add this job after the existing `deploy-pages` job:

```yaml
  deploy-cloudflare:
    name: Deploy to Cloudflare Pages
    needs: build-docs
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6

      # Rebuild artifact in this job (artifacts are not shared cross-job by default)
      - uses: pnpm/action-setup@v6
        with:
          version: 9
      - uses: actions/setup-node@v6
        with:
          node-version: 24
          package-manager-cache: false
      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
      - name: Build Docs Pipeline
        run: cargo build -p vox-doc-pipeline --profile ci
      - name: Run Docs Pipeline
        run: cargo run -p vox-doc-pipeline
        env:
          SOURCE_DATE_EPOCH: ${{ github.event.head_commit.timestamp }}
      - name: Install docs-astro deps
        working-directory: docs-astro
        run: pnpm install --frozen-lockfile
      - name: Build Starlight
        working-directory: docs-astro
        run: pnpm build
      - name: Deploy to Cloudflare Pages
        working-directory: docs-astro
        run: npx wrangler pages deploy dist --project-name=vox-docs --branch=main --commit-dirty=true
        env:
          CLOUDFLARE_API_TOKEN: ${{ secrets.CF_API_TOKEN }}
          CLOUDFLARE_ACCOUNT_ID: ${{ secrets.CF_ACCOUNT_ID }}
```

> **Note on duplicated build:** Yes, this rebuilds the docs in the CF job rather than passing an artifact from `build-docs`. Reasoning: we want the CF deploy to be self-contained so removing the GH Pages job later (Phase Z) doesn't require restructuring. The extra ~3 min is acceptable for the cutover period. After Phase Z we collapse `build-docs` + `deploy-cloudflare` into a single job.

- [ ] **Step 2: Validate YAML locally**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/docs-deploy.yml'))" && echo "YAML OK"
```

Expected: `YAML OK`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/docs-deploy.yml
git commit -m "ci(docs): add parallel Cloudflare Pages deploy job (GH Pages kept for cutover)"
```

---

### Task A.6: First test deploy from local

- [ ] **Step 1: Build locally**

```bash
cd docs-astro && pnpm build
```

- [ ] **Step 2: Deploy via wrangler**

```bash
cd docs-astro && wrangler pages deploy dist --project-name=vox-docs --branch=main --commit-dirty=true
```

Expected: upload of all assets, then a URL like `https://<hash>.vox-docs.pages.dev`.

- [ ] **Step 3: Smoke-test the deployment URL**

```bash
curl -sI https://vox-docs.pages.dev/ | head -5
```

Expected: `HTTP/2 200`. Server header includes `cloudflare`.

Open `https://vox-docs.pages.dev/` in a browser. Verify:
- Site loads
- Sidebar has the new section structure
- A few links work

If anything is wrong, fix and redeploy before proceeding.

---

### Task A.7: Attach custom domains via CF REST API

> Use the token and account ID from Task A.3. Export them once for this shell session.

- [ ] **Step 1: Export credentials**

```bash
export CF_API_TOKEN="<token>"
export CF_ACCOUNT_ID="<account id>"
```

- [ ] **Step 2: Attach voxlang.org**

```bash
curl -fsS -X POST \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"voxlang.org"}' | jq
```

Expected: `"success": true`, domain object returned with `"status":"pending"` initially.

- [ ] **Step 3: Attach www.voxlang.org**

```bash
curl -fsS -X POST \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"www.voxlang.org"}' | jq
```

Expected: `"success": true`.

- [ ] **Step 4: Wait for DNS + TLS provisioning (~1–5 min) and verify**

```bash
curl -fsS \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" | jq '.result[] | {name, status, certificate_authority}'
```

Re-run until both domains show `"status": "active"`.

- [ ] **Step 5: Confirm DNS records exist in voxlang.org zone**

Run in CF dashboard → `voxlang.org` zone → DNS, or:
```bash
dig voxlang.org +short
dig www.voxlang.org +short
```

Expected: both resolve to Cloudflare IPs.

- [ ] **Step 6: Open in browser**

`https://voxlang.org/` should load the same content as `https://vox-docs.pages.dev/`.

---

### Task A.8: Temporarily attach vox-lang.org to CF Pages

This makes `vox-lang.org` serve from CF Pages for a moment before we swap it to a redirect — zero-downtime cutover.

- [ ] **Step 1: Attach vox-lang.org via CF API**

```bash
curl -fsS -X POST \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"vox-lang.org"}' | jq
```

Expected: `"success": true`. (May fail with conflict if the current `CNAME @ → vox-foundation.github.io` record clashes. If so, proceed to **Phase X.1** which removes that record, then retry this step.)

- [ ] **Step 2: Verify**

```bash
curl -fsS \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains" \
  -H "Authorization: Bearer $CF_API_TOKEN" | jq '.result[] | {name, status}'
```

All three of `voxlang.org`, `www.voxlang.org`, `vox-lang.org` should be `active`.

- [ ] **Step 3: Confirm vox-lang.org now serves from CF Pages**

```bash
curl -sI https://vox-lang.org/ | head -5
```

Expected: `HTTP/2 200`, `server: cloudflare`. (Pre-cutover: `x-github-request-id` header was present — should now be gone.)

---

## Phase X: vox-lang.org → voxlang.org Redirect Cutover

This phase replaces `vox-lang.org`'s content-serving role with a 301 redirect to `voxlang.org`.

### Task X.1: Detach vox-lang.org from CF Pages

- [ ] **Step 1: Detach via CF API**

```bash
curl -fsS -X DELETE \
  "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/pages/projects/vox-docs/domains/vox-lang.org" \
  -H "Authorization: Bearer $CF_API_TOKEN" | jq
```

Expected: `"success": true`. CF Pages stops claiming the hostname.

---

### Task X.2: Replace DNS records in vox-lang.org zone (dashboard)

> Manual dashboard steps. Get zone ID first if you prefer API, but dashboard is faster.

- [ ] **Step 1: Open vox-lang.org zone**

CF dashboard → select `vox-lang.org` zone → **DNS** → **Records**.

- [ ] **Step 2: Delete the old GitHub Pages records**

Find and delete:
- `CNAME @ → vox-foundation.github.io` (or any A records for `@` left from CF Pages)
- `CNAME www → vox-foundation.github.io` (if present)
- `CNAME www → vox-lang.org` (if present)

- [ ] **Step 3: Add proxied A records**

Click **Add record** twice and create:
```
Type: A   Name: @     IPv4: 192.0.2.1   Proxy: ✓ Proxied
Type: A   Name: www   IPv4: 192.0.2.1   Proxy: ✓ Proxied
```

- [ ] **Step 4: Verify**

```bash
dig vox-lang.org +short
dig www.vox-lang.org +short
```

Both should return Cloudflare IPs (not 192.0.2.1 — CF is fronting).

---

### Task X.3: Create the Redirect Rule

- [ ] **Step 1: Open Redirect Rules**

CF dashboard → `vox-lang.org` zone → **Rules** → **Redirect Rules** → **Create rule**.

- [ ] **Step 2: Fill in the rule**

- Rule name: `Redirect vox-lang.org → voxlang.org`
- When incoming requests match: **Custom filter expression**
  - Field: `Hostname`
  - Operator: `is in`
  - Value: `vox-lang.org`, `www.vox-lang.org`
- Then:
  - Type: **Dynamic**
  - Expression: `concat("https://voxlang.org", http.request.uri.path)`
  - Status code: `301`
  - Preserve query string: ✓

Click **Deploy**.

- [ ] **Step 3: Verify redirect**

```bash
curl -sI https://vox-lang.org/
curl -sI https://vox-lang.org/getting-started
curl -sI https://www.vox-lang.org/some/deep/path?q=1
```

Expected: each returns `301` (or `308`), with `location:` header pointing at the corresponding `https://voxlang.org/...` URL, query string preserved on the third.

---

## Phase C: Live Site Smoke Test

### Task C.1: Write the smoke test

**Files:**
- Create: `docs-astro/tests/smoke.spec.ts`

- [ ] **Step 1: Write the test file**

`docs-astro/tests/smoke.spec.ts`:
```typescript
import { test, expect } from '@playwright/test';

const PRIMARY = process.env.BASE_URL ?? 'https://voxlang.org';

test.describe('voxlang.org live site', () => {
  test('home page loads with Vox title', async ({ page }) => {
    const resp = await page.goto(PRIMARY + '/');
    expect(resp?.status()).toBe(200);
    await expect(page).toHaveTitle(/Vox/);
  });

  test('sidebar renders new section labels', async ({ page }) => {
    await page.goto(PRIMARY + '/');
    const sidebar = page.locator('starlight-toc, nav[aria-label*="Main" i], aside nav').first();
    await expect(sidebar).toContainText('Getting Started');
    await expect(sidebar).toContainText('How-To Guides');
    await expect(sidebar).toContainText('Tutorials');
    await expect(sidebar).toContainText('Language Reference');
  });

  test('pagefind search is available', async ({ page }) => {
    await page.goto(PRIMARY + '/');
    // Pagefind exposes a #search element or a search input with role=searchbox
    const searchTrigger = page.locator('[data-pagefind-search], button[aria-label*="search" i], input[type="search"]').first();
    await expect(searchTrigger).toBeVisible({ timeout: 10_000 });
  });

  test('www.voxlang.org also serves the site', async ({ request }) => {
    const resp = await request.get('https://www.voxlang.org/', { maxRedirects: 5 });
    expect(resp.status()).toBe(200);
  });

  test('vox-lang.org redirects to voxlang.org with path preserved', async ({ request }) => {
    const resp = await request.get('https://vox-lang.org/getting-started', { maxRedirects: 0 });
    expect([301, 308]).toContain(resp.status());
    const location = resp.headers()['location'];
    expect(location).toMatch(/^https:\/\/voxlang\.org\/getting-started/);
  });

  test('www.vox-lang.org also redirects', async ({ request }) => {
    const resp = await request.get('https://www.vox-lang.org/', { maxRedirects: 0 });
    expect([301, 308]).toContain(resp.status());
    expect(resp.headers()['location']).toMatch(/^https:\/\/voxlang\.org\//);
  });
});
```

- [ ] **Step 2: Commit**

```bash
git add docs-astro/tests/smoke.spec.ts
git commit -m "test(docs-astro): add Playwright smoke tests for voxlang.org cutover"
```

---

### Task C.2: Run smoke test locally against live site

- [ ] **Step 1: Run**

```bash
cd docs-astro && BASE_URL=https://voxlang.org npx playwright test tests/smoke.spec.ts
```

Expected: all 6 tests pass.

- [ ] **Step 2: If any fail, investigate**

For each failure:
- `home page loads` fails → CF Pages deploy not actually live; recheck Task A.7
- `sidebar renders` fails → check if `pnpm build` was run after Task B.4; redeploy
- `pagefind search` fails → may be a selector difference; open browser, inspect the search element, update the selector
- `redirect` fails → recheck Task X.3 (Redirect Rule may not have deployed yet — wait 30s and retry)

Fix and rerun until all 6 pass. Do **not** proceed until smoke test is green.

---

### Task C.3: Wire smoke test into CI

**Files:**
- Modify: `.github/workflows/docs-deploy.yml`

- [ ] **Step 1: Add smoke-test job after deploy-cloudflare**

```yaml
  smoke-test:
    name: Smoke test live site
    needs: deploy-cloudflare
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: pnpm/action-setup@v6
        with:
          version: 9
      - uses: actions/setup-node@v6
        with:
          node-version: 24
          package-manager-cache: false
      - name: Install docs-astro deps
        working-directory: docs-astro
        run: pnpm install --frozen-lockfile
      - name: Install Playwright Chromium
        working-directory: docs-astro
        run: npx playwright install chromium --with-deps
      - name: Wait for CF Pages propagation
        run: sleep 30
      - name: Run smoke test
        working-directory: docs-astro
        run: npx playwright test tests/smoke.spec.ts
        env:
          BASE_URL: https://voxlang.org
```

- [ ] **Step 2: Validate YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/docs-deploy.yml'))" && echo "YAML OK"
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/docs-deploy.yml
git commit -m "ci(docs): run Playwright smoke test against live voxlang.org after each deploy"
```

---

### Task C.4: Merge to main and observe CI

- [ ] **Step 1: Open a PR with all of Phase B + A + C changes** (if not already on main)

```bash
git push origin <branch>
gh pr create --title "docs: migrate to voxlang.org on Cloudflare Pages + fix sidebar" \
  --body "$(cat <<'EOF'
## Summary
- Migrates docs deployment from GitHub Pages to Cloudflare Pages on voxlang.org
- vox-lang.org becomes a 301 redirect to voxlang.org (preserves path + query)
- Realigns frontmatter category values across ~450 files with sidebar SSOT labels
- Adds Playwright smoke tests that run post-deploy

## Test plan
- [ ] Local `pnpm build` succeeds in docs-astro
- [ ] Local Playwright smoke test against live voxlang.org passes
- [ ] CI green
- [ ] Verify post-merge: docs-deploy workflow completes, smoke-test job passes
- [ ] Spot-check sidebar renders correctly on https://voxlang.org/
EOF
)"
```

- [ ] **Step 2: Merge once CI is green**

After merge:
```bash
gh run watch
```

Expected: `build-docs`, `deploy-pages`, `deploy-cloudflare`, and `smoke-test` all green.

- [ ] **Step 3: Visit live site manually**

Open `https://voxlang.org/` in a browser. Click through 3–5 sidebar sections. Spot-check no broken links.

---

## Phase Z: Cleanup (run only after 48h of stable CF Pages operation)

### Task Z.1: Remove GitHub Pages deploy from workflow

**Files:**
- Modify: `.github/workflows/docs-deploy.yml`

- [ ] **Step 1: Remove the GH Pages-related blocks**

Delete:
- The `permissions: pages: write, id-token: write` block
- The `Upload Pages artifact` step in `build-docs`
- The entire `deploy-pages` job

The `build-docs` and `deploy-cloudflare` jobs can also be collapsed into one job to avoid the duplicate build (move all build steps into `deploy-cloudflare`, delete `build-docs`).

- [ ] **Step 2: Validate YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/docs-deploy.yml'))" && echo "YAML OK"
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/docs-deploy.yml
git commit -m "ci(docs): retire GitHub Pages deploy; Cloudflare Pages is now sole target"
```

---

### Task Z.2: Remove CNAME files

**Files:**
- Delete: `./CNAME`
- Delete: `docs-astro/public/CNAME`

- [ ] **Step 1: Delete both files**

```bash
git rm ./CNAME docs-astro/public/CNAME
```

- [ ] **Step 2: Build and confirm dist doesn't need them**

```bash
cd docs-astro && pnpm build && ls dist/ | grep -i cname
```

Expected: no CNAME file in dist (Cloudflare Pages doesn't use it).

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: remove GitHub Pages CNAME files (no longer used)"
```

---

### Task Z.3: Disable GitHub Pages in repo settings

- [ ] **Step 1: Open GitHub repo → Settings → Pages**

Change "Build and deployment" → Source: select **None** (or "Disable GitHub Pages" if available).

This stops GitHub from generating Pages URLs for the repo.

- [ ] **Step 2: Confirm vox-foundation.github.io/vox no longer resolves to the docs**

```bash
curl -sI https://vox-foundation.github.io/vox/
```

Expected: `404` after a few minutes (GH may cache for a bit).

---

## Self-Review

**Spec coverage check** (Done — each spec section has a task):
- ✓ A1 (wrangler login) → Task A.1
- ✓ A2 (CF Pages project) → Task A.2
- ✓ A3 (CI deploy step) → Task A.5
- ✓ A4 (custom domains via curl) → Task A.7
- ✓ A5 (redirect rule + DNS) → Tasks X.2, X.3
- ✓ A6 (astro.config.mjs) → Task A.4
- ✓ B1 (new taxonomy) → Task B.4
- ✓ B2 (.vox script) → Tasks B.1, B.2, B.3
- ✓ B3 (SSOT update) → Task B.4
- ✓ B4 (validation gates) → Task B.5
- ✓ C1 (baseline) → Tasks 0.1, 0.2, 0.3
- ✓ C2 (smoke test file) → Task C.1
- ✓ C3 (CI integration) → Task C.3
- ✓ Cutover ordering → Phase X
- ✓ Cleanup → Phase Z

**Placeholder scan** — no TBD, TODO, "add appropriate handling", or "similar to Task N" found.

**Type/identifier consistency** — project name `vox-docs` used consistently; secret names `CF_API_TOKEN` and `CF_ACCOUNT_ID` consistent across A.3, A.5, A.7; Playwright config exports `BASE_URL` consistently.
