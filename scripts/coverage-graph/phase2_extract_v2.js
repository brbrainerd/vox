export const meta = {
  name: 'phase2-coverage-behavior-extraction-v2',
  description: 'Throttled re-run of per-crate proven-behavior extraction for rate-limited crates (chunked)',
  phases: [
    { title: 'Extract', detail: 'sequential chunks of 8 crates, parallel within a chunk' },
  ],
}

const REPO = 'C:/Users/Owner/vox'
const crates = typeof args === 'string' ? JSON.parse(args) : args
const CHUNK = 8

const BEHAVIOR_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    crate: { type: 'string' },
    behaviors: {
      type: 'array',
      items: {
        type: 'object',
        additionalProperties: false,
        properties: {
          file: { type: 'string', description: 'path containing crates/<crate>/ — source file of the symbol if identifiable, else the test file' },
          about: { type: 'string', description: 'the production symbol asserted on, e.g. Type::method or func()' },
          claim: { type: 'string', description: 'one sentence: the proven observable behavior' },
          kind: { type: 'string', enum: ['happy', 'error', 'edge', 'invariant'] },
          confidence: { type: 'string', enum: ['EXTRACTED', 'INFERRED', 'AMBIGUOUS'] },
        },
        required: ['file', 'about', 'claim', 'kind', 'confidence'],
      },
    },
  },
  required: ['crate', 'behaviors'],
}

function prompt(crate) {
  return `You are extracting PROVEN test behaviors for the Rust crate \`${crate}\` in the vox workspace rooted at ${REPO}.

GOAL: build the semantic test-coverage map — what the test suite actually *asserts*, not merely what it touches. The keystone signal is the gap between symbols proven only on the happy path vs. those with error/edge/invariant proofs.

STEPS:
1. Locate this crate's test code (use Glob/Grep/Read with absolute paths under ${REPO}/crates/${crate}/):
   - unit/integration tests inside #[cfg(test)] modules: files in ${REPO}/crates/${crate}/src/** containing #[test] / #[tokio::test] / #[rstest].
   - external tests: every .rs file under ${REPO}/crates/${crate}/tests/**.
2. Read the test bodies. For EACH test, determine the production symbol(s) whose OBSERVABLE BEHAVIOR is asserted — the value or effect flowing into assert!/assert_eq!/assert_ne!/expect()/unwrap on a checked value/matches!/snapshot or golden compare. A symbol merely called but never asserted on does NOT count.
3. Emit one behavior claim per (symbol, distinct asserted property):
   - file: a path that CONTAINS \`crates/${crate}/\` (prefer the symbol's source file; the test file is acceptable). Routing requires this.
   - about: the symbol, e.g. \`Foo::bar\` or \`parse_module()\` or a bare type name.
   - claim: ONE sentence describing the proven behavior in plain language.
   - kind: 'happy' = nominal/success path; 'error' = an Err/panic/failure path is asserted; 'edge' = boundary/empty/overflow/duplicate; 'invariant' = a property asserted to always hold (ordering, idempotence, round-trip, monotonicity).
   - confidence: EXTRACTED = directly evident; INFERRED = strongly implied; AMBIGUOUS = uncertain.

RULES:
- Never invent a claim. If a test asserts nothing meaningful, skip it.
- Prefer breadth across DISTINCT asserted symbols. error/edge/invariant claims are the most valuable — capture every one you find.

Return ONLY the structured object { crate, behaviors: [...] }. Your structured output IS the result; do not write any files.`
}

const all = []
for (let i = 0; i < crates.length; i += CHUNK) {
  const chunk = crates.slice(i, i + CHUNK)
  log(`chunk ${i / CHUNK + 1}/${Math.ceil(crates.length / CHUNK)}: ${chunk.join(', ')}`)
  const res = await parallel(
    chunk.map((c) => () =>
      agent(prompt(c), { label: `extract:${c}`, phase: 'Extract', schema: BEHAVIOR_SCHEMA })
        .then((r) => ({ crate: c, n: r && r.behaviors ? r.behaviors.length : 0 }))
    )
  )
  all.push(...res.filter(Boolean))
}

const total = all.reduce((s, r) => s + r.n, 0)
const empty = all.filter((r) => r.n === 0).map((r) => r.crate)
log(`v2 extraction complete: ${total} claims; ${empty.length} still empty`)
return { totalClaims: total, crates: all.length, emptyCrates: empty, perCrate: all }
