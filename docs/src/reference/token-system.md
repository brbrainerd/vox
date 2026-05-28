---
title: "Design token system"
description: "How vox.tokens.json is validated by the compiler and how tokens map to CSS custom properties."
category: "Language Reference"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# Design token system

Vox compiles `vox.tokens.json` into typed CSS custom properties and validates every token
reference at compile time. Unknown token names are compile errors; raw color literals on
color-accepting properties are warnings.

## Token file shape

Token files must conform to `contracts/tokens/tokens.v1.json`. The minimal structure:

```json
{
  "$schema": "./contracts/tokens/tokens.v1.json",
  "color": {
    "primary": "#3a86ff",
    "background": "#ffffff",
    "text": {
      "value": "#1d3557",
      "on": "color.background",
      "text_role": "body"
    }
  },
  "spacing": { "md": "16px" },
  "font": { "base": "16px" }
}
```

## CSS custom property mapping

Token paths become CSS custom properties by joining segments with `-` and prefixing `--vox-`:

| JSON path | CSS custom property |
|---|---|
| `color.primary` | `--vox-color-primary` |
| `color.text` | `--vox-color-text` |
| `spacing.md` | `--vox-spacing-md` |

In Vox source use `tokens.<path>` syntax:

```vox
// vox:skip
component Button {
  style {
    color: tokens.color.text
    background: tokens.color.primary
  }
}
```

## Contrast validation

Color tokens with `on` and `text_role` metadata are validated against WCAG 2.1 §1.4.3 at
token-load time:

| `text_role` | Warn below | Error below |
|---|---|---|
| `body` | 4.5:1 | 3:1 |
| `large` | 3:1 | 3:1 |
| `ui` | 3:1 | 3:1 |

Validation runs via `TokenRegistry::validate_contrast()` in
`crates/vox-compiler/src/tokens/mod.rs`. The contrast algorithm is WCAG 2.1 relative
luminance (IEC 61966-2-1 sRGB linearization) implemented in
`crates/vox-compiler/src/tokens/contrast.rs`.

## Diagnostic codes

| Code | Severity | Meaning |
|---|---|---|
| `web_ir_validate.style.unknown_token` | error | TokenRef not present in `vox.tokens.json` |
| `web_ir_validate.style.raw_color_value` | warning | Raw hex/rgb/named color on a color property |
| `web_ir_validate.style.token_contrast_warning` | warning | Declared contrast pair below warn threshold |
| `web_ir_validate.style.token_contrast_error` | error | Declared contrast pair below error threshold |

## Contract

The normative machine-readable contract is
[`contracts/tokens/tokens.v1.json`](../../../contracts/tokens/tokens.v1.json) (JSON Schema
Draft 2020-12).

## Related

- [`LANGUAGE_DESIGN_PRIORITIES.md`](../../../LANGUAGE_DESIGN_PRIORITIES.md) — P0 priority that motivates compile-time token enforcement (C2: GUI wedge)
- [GUI-native roadmap status](../architecture/gui-native-roadmap-status-2026.md) — TASK-4.4
