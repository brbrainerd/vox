# Vox Golden Example Style Guide

This document defines the coding conventions, frontmatter requirements, and structural rules for all `.vox` files in the `examples/` directory. All contributors and AI agents must follow these rules.

---

## 1. Frontmatter (Required)

Every `.vox` file **must** begin with a structured comment-frontmatter block. This block is parsed by the Vox CI pipeline and the Mens training corpus ingestion pipeline.

```
// ---
// title: "Descriptive Example Title"
// description: "120-158 character description of what this example demonstrates."
// syntax_version: "0.5.0"
// status: golden | deprecated | archived | experimental
// category: example
// constructs: [actor, workflow, server, table, fn, test]
// last_validated: YYYY-MM-DD
// training_eligible: true | false
// training_weight: 0.0 - 2.0
// difficulty: beginner | intermediate | advanced
// supersedes: ""
// superseded_by: ""
// ---
```

### Field Definitions

| Field | Required | Description |
|-------|----------|-------------|
| `title` | Yes | Human-readable title with keywords. |
| `description` | Yes | 120-158 chars describing what is demonstrated. |
| `syntax_version` | Yes | The Vox version this file targets (e.g., `"0.5.0"`). |
| `status` | Yes | See lifecycle table below. |
| `category` | Yes | Always `example` for `.vox` files. |
| `constructs` | Yes | Vox constructs demonstrated. Used for training and search. |
| `last_validated` | Yes | ISO date when `vox check` last succeeded. Updated by CI. |
| `training_eligible` | Yes | `true` for `golden` only. All others: `false`. |
| `training_weight` | Yes | `1.0` default. `1.5-2.0` for exemplary examples. `0.0` for deprecated. |
| `difficulty` | Yes | `beginner`, `intermediate`, or `advanced`. |
| `supersedes` | No | Filename of the older file this replaces. Empty string if none. |
| `superseded_by` | No | Filename of the newer file. Set when deprecating. |

---

## 2. File Naming

- Use `snake_case.vox` (e.g., `crud_api.vox`, `counter_actor.vox`).
- Names must describe the concept demonstrated, not be generic (never `example1.vox`).
- Files in `golden/` are the canonical source of truth. `archive/` contains deprecated files.

---

## 3. Length & Focus

- Each file should be **20 to 100 lines** of Vox code.
- Demonstrate **1-3 related constructs** per file. Do not pack every feature into one example.
- Files must be **self-contained** — they must compile and pass `vox check <file>` independently.

---

## 4. Comments

- Use `//` line comments to explain non-obvious constructs.
- Do **not** over-annotate. Obvious code (variable assignments, simple returns) needs no comment.
- Every example should include at least one explanatory comment block explaining the core concept.

---

## 5. Testing

- Include at least one `@test` function that validates the example's output.
- Tests make examples runnable in CI and provide a correctness guarantee for Mens training.

---

## 6. Lifecycle Transitions

When a new Vox syntax version breaks an existing golden example:

1. Create the new corrected example in `golden/` with `supersedes: "old_file.vox"`.
2. Add `superseded_by: "new_file.vox"` and `status: deprecated` to the old file's header.
3. Set `training_eligible: false` and `training_weight: 0.0` on the deprecated file.
4. Move the deprecated file to `archive/` directory.
5. CI will update `PARSE_STATUS.md` automatically on the next run.

---

## 7. Directory Layout

```
examples/
├── golden/        # Current, validated examples (status: golden)
├── archive/       # Deprecated or superseded examples (status: deprecated|archived)
├── experimental/  # Forward-looking examples for unreleased syntax (status: experimental)
├── STYLE.md       # This file
├── PARSE_STATUS.md  # CI-generated parse matrix
└── README.md      # Overview and lifecycle explanation
```
