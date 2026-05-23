---
title: "vox share — Abuse Policy and ToS Reference (2026)"
description: "Terms of Service references, abuse contacts, takedown policy, and privacy notes for the vox share tunnel feature."
category: "Architecture SSOTs"
last_updated: "2026-05-09"
status: "current"
training_eligible: true

schema_type: "TechArticle"
keywords: ["vox share", "abuse policy", "terms of service", "takedown", "privacy", "Cloudflare tunnel", "localhost.run", "Tailscale"]
---
# vox share — Abuse Policy and ToS Reference (2026)

## What `vox share` does

`vox share` packages and invokes third-party tunnel clients on the user's machine. Traffic flows directly between the user's machine and the tunnel provider's edge — Vox does not proxy, relay, or inspect traffic.

The Vox project's role is limited to distributing the launcher software that invokes these third-party services. All network connectivity is established and operated by the selected tunnel provider.

## Terms of Service references

Users are responsible for complying with the applicable Terms of Service when using `vox share`.

| Provider | ToS |
|---|---|
| Cloudflare Quick Tunnels (default) | [Cloudflare Online Services Terms](https://www.cloudflare.com/website-terms/) |
| localhost.run | [localhost.run Terms](https://localhost.run/docs/faq) |
| Tailscale Funnel | [Tailscale Terms of Service](https://tailscale.com/terms) |

On first use, `vox share` displays a one-time prompt requiring the user to acknowledge these terms. This prompt can be suppressed in non-interactive environments with `--accept-tos`.

## Abuse and takedown

Because Vox does not operate the relay infrastructure, abuse complaints about content served via `vox share` should be directed to the tunnel provider:

- **Cloudflare:** abuse@cloudflare.com
- **localhost.run:** see [localhost.run FAQ](https://localhost.run/docs/faq)
- **Tailscale:** security@tailscale.com

If an abuse report is received by Anthropic or the Vox project, we will acknowledge receipt and direct the reporter to the appropriate tunnel provider. We do not have the ability to terminate individual tunnel sessions or inspect their traffic.

## Privacy

- **Cloudflare** and **localhost.run** may log connection metadata (IP addresses, timestamps, domain names) in accordance with their respective privacy policies.
- **Tailscale Funnel** traffic is subject to [Tailscale's Privacy Policy](https://tailscale.com/privacy-policy).
- **LAN backend** traffic stays on the local network and is not routed through any third-party service.
- The shared app itself may log user interactions. App-level logging and data handling are the responsibility of the app developer.

Vox does not collect or store any metadata about `vox share` sessions.

## Stability

Cloudflare Quick Tunnels are a free, no-SLA service. Session URLs are ephemeral (unique per session, not persistent). If Cloudflare tightens access controls or is unreachable, `vox share` automatically falls back to localhost.run. For stable, persistent URLs, use `--backend tailscale` (requires an active Tailscale account).
