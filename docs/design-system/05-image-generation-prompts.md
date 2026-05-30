# 05 — Image generation prompts
_Last reviewed: 2026-05-30. Per-asset prompts for Imagen / FLUX / Midjourney. Run separately from the page prompts._

The page prompts (`01`–`03`) emit `/api/placeholder/W/H` paths. This file holds the prompts that produce the **real** images those placeholders stand in for. Run these in an image model, then the integration step rewrites the placeholder paths to `/assets/...` where a matching asset exists (see `01` § "Integration after the artifact returns").

These are art-direction prompts, not React. Keep them aligned to the palette in `04-visual-style-guide.md` § palette so generated imagery reads as the same brand: warm paper background, orange (`hsl(16 100% 60%)`) and teal (`hsl(167 42% 66%)`) accents, ink near-black, no neon, no stock-photo gloss.

**Shared style suffix** — append to every prompt below unless it conflicts:

> Editorial illustration, flat vector with subtle grain, warm off-white paper background (#FAF8F3), orange and muted-teal accent palette, near-black ink linework, generous negative space, no text, no logos, no photorealism, no gradients-to-neon. 4:3 unless noted.

---

## Hero — "compilation as a prism"

Target: landing-page hero right column. Alt text the page uses: "Compilation as a prism: one source unfolding into database, server, and browser UI layers."

> A single beam of warm orange light entering a glass prism from the left, refracting into three distinct labeled streams flowing right: one becomes a stack of database cylinders, one a server rack outline, one a browser window with UI cards. Flat editorial vector, ink linework, teal highlights on the split streams. The single input is visually emphatic; the three outputs are calm and ordered. 8:6 (landscape).

Size: 800×600.

---

## Concepts — "three layers collapse into one file"

Target: `/concepts/` explainer. For non-programmers.

> Three translucent paper sheets labeled (by shape, not text) database, server, interface, stacked and merging downward into a single solid sheet with one orange seam down the middle. The merge reads as simplification, not compression-by-force. Soft drop shadow, warm paper ground. 4:3.

Size: 1200×900.

---

## Showcase — per-demo thumbnails (8)

Target: `/showcase/` gallery cards. One small square per demo. Keep them a consistent set — same linework weight, same two-accent palette — so the grid reads as a family.

Generate with this template, substituting the motif:

> Small square editorial icon-illustration on warm paper, single orange focal accent with one teal supporting element, thick ink linework, centered, lots of margin. Motif: {MOTIF}. No text.

| # | Demo | `{MOTIF}` |
|---|------|-----------|
| 1 | `@table` schema | a single declaration sprouting a small schema tree |
| 2 | Errors as values | a `Result` envelope splitting into an Ok check and an Error cross |
| 3 | Phonetic operators | speech-bubble glyphs morphing into `is` / `and` / `or` tokens |
| 4 | Durable workflow | a looping arrow with a journal/ledger underneath, resuming after a break |
| 5 | MCP tool | a typed function plug connecting to a generic client socket |
| 6 | React interop | a `component` block unfolding into a browser card |
| 7 | MENS local training | a small GPU chip with a QLoRA adapter clipped on, no cloud |
| 8 | One-file build | one document fanning into schema + server + client glyphs |

Size: 480×480 each.

---

## Persona portraits (3)

Target: `01` § "Who Vox is for". Abstract, not real faces — the kit avoids implying specific people.

> Abstract head-and-shoulders silhouette in flat vector, filled with a motif that signals the role, warm paper ground, single accent. Motif: {MOTIF}. No facial features, no text, gender-neutral.

| Persona | `{MOTIF}` |
|---------|-----------|
| Backend engineer | the silhouette filled with a clean schema-and-pipe diagram |
| Agentic builder | the silhouette filled with a small graph of connected tool nodes |
| Researcher | the silhouette filled with a compact training-curve and chip motif |

Size: 80×80 each (generate at 320×320 and downscale for crispness).

---

## Open-graph / social card

Target: `og:image` for link previews. This one **may include text** (the only asset that does).

> Centered wordmark "Vox" in a serif display face on warm paper, with the tagline "The AI-native programming language." beneath in a clean sans. A faint prism motif (see Hero) in the lower-right corner, orange-and-teal. Balanced margins, no clutter. 1200×630.

Size: 1200×630.

---

## Running these

1. Pick a model (Imagen, FLUX, or Midjourney). Midjourney: append `--ar W:H --style raw`. FLUX/Imagen: pass the aspect ratio as the model expects.
2. Generate, pick the cleanest result, export at the target size.
3. Drop into the site asset directory; the integration script rewrites the matching `/api/placeholder/W/H` path to the real `/assets/...` path.
4. If an asset drifts from the palette, fix the prompt here and regenerate — do not hand-edit the image. The prompt is the SSOT, same rule as the page prompts.
</content>
