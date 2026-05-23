---
title: "How-To: Share a Vox App"
description: "Expose a running Vox app on a public URL in one command using vox share, with automatic tunnel selection and optional authentication."
category: "How-To Guides"
last_updated: "2026-05-09"
status: "current"
training_eligible: true

schema_type: "HowTo"
keywords: ["vox share", "share Vox app", "public URL", "Cloudflare tunnel", "localhost.run", "Tailscale Funnel", "LAN share"]
---
# How-To: Share a Vox App

`vox share` exposes a running Vox app on a public URL in one command. It defaults to Cloudflare Quick Tunnels and automatically falls back to localhost.run if Cloudflare is unavailable.

## Quick start

```sh
vox share app.vox           # builds + shares (Cloudflare default)
vox share app.vox --dev     # shares the live dev server (faster iteration)
vox share --port 7860       # shares an already-running app on port 7860
```

On first run, `vox share` displays a one-time Terms of Service prompt. Pass `--accept-tos` to skip it in CI or non-interactive environments.

## Backends

| Backend | Flag | URL stability | SSE support |
|---|---|---|---|
| Cloudflare Quick Tunnel (default) | `--backend cloudflare` | Per-session | Limited (200 concurrent) |
| localhost.run | `--backend localhost-run` | Per-session | Yes |
| Tailscale Funnel | `--backend tailscale` | Stable (`*.ts.net`) | Yes |
| LAN only | `--backend lan` | Stable (LAN IP) | Yes |

`vox share` selects the backend automatically based on availability. You can force a specific backend with the `--backend` flag.

## Authentication

By default, the shared URL includes an embedded token (`?vox_share_token=...`). Anyone with the link can access the app; without the token, requests return a 401.

- `--auth none` — disable auth (public, no token required)
- `--auth basic:user:pass` — HTTP Basic auth

## Duration

The session auto-shuts down after 8 hours by default.

- `--duration 30m` — shut down after 30 minutes
- `--duration none` — run indefinitely

Press `Ctrl-C` at any time to stop the session immediately.

## SSE / Streaming

If your app uses Server-Sent Events (SSE, e.g., streaming LLM responses), `vox share` automatically switches to localhost.run when Cloudflare is selected, because Cloudflare Quick Tunnels buffer SSE connections. Use `--allow-buffered-streaming` to keep Cloudflare if buffering is acceptable for your use case.

## Common pitfalls

- **Corporate network blocks `*.trycloudflare.com`** — try `--backend tailscale` or `--backend localhost-run`
- **Port already in use** — pass `--port <other-port>` to the app and match it in `vox share --port <other-port>`
- **CI / non-interactive** — pass `--accept-tos` to skip the first-run consent prompt
- **Tailscale not installed** — the Tailscale backend requires `tailscale` on `$PATH` and an active Tailscale session

## Safety

Shared URLs are accessible to anyone who has the link. For demos with sensitive data, use `--auth basic:user:pass`, or use `--auth none` and share the link only over a trusted channel. The shared session terminates automatically after the configured duration.

See [share-policy-2026](../architecture/share-policy-2026.md) for ToS references, abuse contacts, and privacy details.
