# Security Policy

The Vox project is licensed under [Apache-2.0](LICENSE). We take security
seriously and appreciate responsible disclosure.

## Supported Versions

Security fixes are applied to the latest release on the default branch. Older
release branches may receive backports at maintainer discretion.

| Version | Supported          |
| ------- | ------------------ |
| 0.6.x   | :white_check_mark: |
| < 0.6   | :x:                |

## Reporting a Vulnerability

**Please do not open public GitHub issues for security vulnerabilities.**

Choose one of these channels:

1. **Email (preferred):** [security@vox-foundation.org](mailto:security@vox-foundation.org)
2. **GitHub private advisory:** Use [Report a vulnerability](https://github.com/vox-foundation/vox/security/advisories/new) on the repository Security tab.

Include as much detail as you can: affected component, reproduction steps, impact
assessment, and any suggested fix.

## Response Timeline

| Milestone              | Target                          |
| ---------------------- | ------------------------------- |
| Initial acknowledgment | Within 3 business days          |
| Triage and severity    | Within 7 business days          |
| Fix or mitigation plan | Depends on severity; we will keep you informed |
| Public disclosure      | Within **90 days** of report, coordinated with the reporter |

We may request an extension for complex issues; we will agree on timing with you
before any public disclosure.

## Disclosure Policy

- We follow a **90-day coordinated disclosure** window from initial report unless
  both parties agree otherwise.
- Credit is given to reporters who wish to be acknowledged (unless you prefer
  anonymity).
- Fixed issues are published via GitHub Security Advisories and release notes.

## Scope

In scope: this repository, official release binaries published from
`.github/workflows/release-binaries.yml`, and documented deployment surfaces.

Out of scope: third-party services, user-hosted configurations, and issues
already fixed on the default branch.
