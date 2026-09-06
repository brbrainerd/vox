---
title: "Legacy URL Redirects on GitHub Pages"
description: "How the Vox docs site handles old mdBook-era URLs via static HTML stubs on GitHub Pages."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---

# Legacy URL Redirects on GitHub Pages

## Problem

`docs-astro/public/_redirects` contains Netlify/Cloudflare Pages redirect rules that map old mdBook `.html` URLs to new Starlight trailing-slash URLs. **GitHub Pages ignores this file entirely** — the `_redirects` format is only processed by Netlify and Cloudflare Pages.

## Why _redirects is a no-op here

The site deploys via `.github/workflows/docs-deploy.yml` using `actions/deploy-pages@v4` — pure GitHub Pages static hosting. There is no Cloudflare Pages adapter in `astro.config.mjs` (no `@astrojs/cloudflare`), no `wrangler.toml`, and no Cloudflare Workers configuration anywhere in the repository. GitHub Pages serves files verbatim and has no redirect-rule processing layer.

## Current fix: static HTML stub files

For the specific legacy routes, `docs-astro/public/` now contains static HTML stubs that GitHub Pages serves as ordinary files. Each stub uses `<meta http-equiv="refresh">` to redirect the browser client-side and a `<link rel="canonical">` to signal the correct URL to crawlers.

Files created under `docs-astro/public/`:

```text
tutorials/tut-getting-started.html
tutorials/tut-first-app.html
tutorials/tut-actor-basics.html
tutorials/tut-workflow-durability.html
tutorials/tut-ui-integration.html
reference/cli.html
reference/ref-syntax.html
reference/ref-decorators.html
reference/ref-type-system.html
reference/ref-stdlib.html
reference/env-vars.html
reference/clavis-ssot.html
reference/secrets-ssot.html
architecture/architecture-index.html
architecture/research-index.html
architecture/research-index/index.html
contributors/contributor-hub.html
```

Both architecture indexes (`architecture/research-index.html`, `architecture/research-index/index.html`, and `architecture/architecture-index.html`) redirect to `/contributors/contributor-hub/`, not to each other.

Astro's build copies everything in `public/` verbatim to `dist/`, so these files land at the correct paths without any build-time configuration changes.

## Unhandled rule: /book/* wildcard

The `_redirects` file also contains:

```text
/book/*   /:splat   301
```

This wildcard pattern **cannot** be served with static stubs — there is no finite set of files that covers an arbitrary suffix. If `/book/` traffic materialises (e.g. from cached CDN links to an old mdBook deployment), the options are:

1. **Cloudflare proxy + Page Rule / Transform Rule** — add a bulk redirect in the Cloudflare dashboard if `voxlang.org` DNS is proxied through Cloudflare.
2. **Cloudflare Worker** — a small Worker script can intercept requests matching `/book/*` and issue a 301 to the path with the prefix stripped.
3. **Accept the 404** — if `/book/` links are not meaningfully indexed or linked, the cost of the broken redirect is low.

The decision requires knowing whether the domain's DNS A/AAAA records are proxied (orange-cloud) in Cloudflare, which is not visible in this repository.

## If migrating away from GitHub Pages

Switching to Cloudflare Pages would allow the existing `_redirects` file to work as-is (including the wildcard rule), and the static stub files could be removed. The only required change would be updating the deployment workflow to use the Cloudflare Pages action instead of `actions/deploy-pages@v4`.
