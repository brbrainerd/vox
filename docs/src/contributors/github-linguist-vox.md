---
title: "GitHub Linguist — Vox language"
description: "How to register Vox with GitHub Linguist and submit the tree-sitter-vox grammar upstream."
category: "Contributors"
status: "current"
training_eligible: true
training_rationale: "Contributor workflow for Vox language recognition on GitHub."
---

# GitHub Linguist — Vox language

GitHub uses [Linguist](https://github.com/github-linguist/linguist) to classify repository languages for stats, syntax highlighting, and search. Vox source (`.vox`) should appear as **Vox**, not as generic text or another language.

## Local repo configuration (done here)

Root [`.gitattributes`](../../../.gitattributes) marks every `.vox` file:

```gitattributes
*.vox text eol=lf linguist-language=Vox linguist-detectable=true
```

- `linguist-language=Vox` — force the display name in GitHub language breakdowns.
- `linguist-detectable=true` — include `.vox` in language statistics (override Linguist heuristics that might skip small or mixed extensions).

This is effective on GitHub once the attributes file is on the default branch. It does **not** require an upstream Linguist merge, but upstream registration improves highlighting defaults and cross-repo consistency.

## Upstream submission checklist

Linguist accepts new languages when they have a real grammar, reasonable adoption, and maintainer responsiveness. Vox already ships a Tree-sitter grammar at [`tree-sitter-vox`](../../../tree-sitter-vox).

### 1. Prepare the grammar artifact

- Grammar source: [`tree-sitter-vox/grammar.js`](../../../tree-sitter-vox/grammar.js)
- Vocabulary SSOT: [`tree-sitter-vox/GRAMMAR_SSOT.md`](../../../tree-sitter-vox/GRAMMAR_SSOT.md)
- Regenerate when the compiler vocabulary changes: `vox grammar --format ssot-markdown --output tree-sitter-vox/GRAMMAR_SSOT.md` (see [`vox ci grammar-ssot-parity`](../reference/cli.md)).

Ensure `tree-sitter-vox` builds cleanly:

```bash
cd tree-sitter-vox
npm install
npx tree-sitter generate
npx tree-sitter test
```

### 2. Fork github/linguist

1. Fork [github/linguist](https://github.com/github/linguist).
2. Add `vendor/grammars/tree-sitter-vox` as a git submodule pointing at this repo’s `tree-sitter-vox` directory (or the published mirror once split).
3. Edit `languages.yml` with a new entry:

```yaml
Vox:
  type: programming
  color: "#5B4FCF"
  extensions:
    - ".vox"
  tm_scope: source.vox
  ace_mode: text
  codemirror_mode: null
  codemirror_mime_type: text/x-vox
  language_id: <next free id — check linguist languages.yml>
```

4. Add a **sample** under `samples/Vox/` (≥25 lines of representative `.vox` from `examples/golden/`).
5. Run Linguist’s test suite locally: `script/cibuild` or `bundle exec rake test` per upstream docs.

### 3. Open the pull request

- Title: `Add Vox language`
- Link to [`tree-sitter-vox`](../../../tree-sitter-vox) and the Vox project site.
- Note that the canonical compiler is [`vox-compiler`](../../../crates/vox-compiler) (recursive descent); Tree-sitter is for highlighting and editor tooling.
- Respond to reviewer feedback on sample size, grammar conflicts, and `language_id` allocation.

### 4. After merge

- Remove or relax repo-level `linguist-language` overrides only if upstream detection is reliable.
- Keep `.gitattributes` `text eol=lf` for cross-platform line endings regardless of Linguist status.

## Related

- [Editor integrations](../how-to/editor-integrations.md) — VS Code, Tree-sitter, LSP
- [Parser ambiguity inventory](../reference/parser-ambiguity-inventory.md) — compiler vs Tree-sitter roles
- [`tree-sitter-vox/README.md`](../../../tree-sitter-vox/README.md) — local grammar build steps
