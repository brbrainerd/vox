# Hardcoded values audit — production code

- **Captured**: 2026-05-12T06:25:19.520Z
- **Total itemized findings**: 1392
- **Total raw matches (pre-cap)**: 5449
- **Cap per category**: 100

## For follow-up LLMs

1. Open `findings.v1.json` — each entry is self-contained.
2. Run `verification_command` from the repository root; expect at least one match.
3. For `confidence: "low"`, confirm in IDE before editing.
4. Regenerate: `node tools/hardcoded-values-audit.mjs .vox\audit\latest`

## Category index

- [01-hardcoded-urls.md](./01-hardcoded-urls.md) — raw: 400, itemized: 100 (cap)
- [02-hardcoded-ports.md](./02-hardcoded-ports.md) — raw: 73, itemized: 73
- [03-hardcoded-ips.md](./03-hardcoded-ips.md) — raw: 88, itemized: 88
- [04-hardcoded-filesystem-paths.md](./04-hardcoded-filesystem-paths.md) — raw: 20, itemized: 20
- [05-hardcoded-timeouts.md](./05-hardcoded-timeouts.md) — raw: 151, itemized: 100 (cap)
- [06-hardcoded-retry-counts.md](./06-hardcoded-retry-counts.md) — raw: 43, itemized: 43
- [07-hardcoded-buffer-sizes.md](./07-hardcoded-buffer-sizes.md) — raw: 119, itemized: 100 (cap)
- [08-hardcoded-version-strings.md](./08-hardcoded-version-strings.md) — raw: 1086, itemized: 100 (cap)
- [09-hardcoded-model-names.md](./09-hardcoded-model-names.md) — raw: 60, itemized: 60
- [10-hardcoded-env-var-names.md](./10-hardcoded-env-var-names.md) — raw: 86, itemized: 86
- [11-brittle-string-needles.md](./11-brittle-string-needles.md) — raw: 2317, itemized: 100 (cap)
- [12-brittle-regex-patterns.md](./12-brittle-regex-patterns.md) — raw: 334, itemized: 100 (cap)
- [13-hardcoded-extensions-globs.md](./13-hardcoded-extensions-globs.md) — raw: 61, itemized: 61
- [14-hardcoded-date-literals.md](./14-hardcoded-date-literals.md) — raw: 57, itemized: 57
- [15-hardcoded-ansi-codes.md](./15-hardcoded-ansi-codes.md) — raw: 5, itemized: 5
- [16-hardcoded-magic-numbers.md](./16-hardcoded-magic-numbers.md) — raw: 312, itemized: 100 (cap)
- [17-hardcoded-canonical-keys.md](./17-hardcoded-canonical-keys.md) — raw: 138, itemized: 100 (cap)
- [18-hardcoded-provider-names.md](./18-hardcoded-provider-names.md) — raw: 91, itemized: 91
- [19-hardcoded-test-data-in-prod.md](./19-hardcoded-test-data-in-prod.md) — raw: 8, itemized: 8
- [20-hardcoded-retired-runtime-names.md](./20-hardcoded-retired-runtime-names.md) — raw: 0, itemized: 0

## Schema

- [findings.v1.schema.json](./findings.v1.schema.json)

## Methodology

- [methodology.md](./methodology.md)
