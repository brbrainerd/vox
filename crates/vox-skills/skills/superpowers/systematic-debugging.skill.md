---
name: systematic-debugging
description: Use when encountering any bug, test failure, or unexpected behavior
---

# Systematic Debugging (Vox Adaptation)

Root cause investigation before any fix attempt. Symptom fixes are failure.

## Phase 1: Investigation

1. **Read Errors:** Analyze stack traces and line numbers.
2. **Reproduce:** Trigger the failure reliably via a minimal test case.
3. **Trace Data:** Use `grep_search` and `view_file` to trace bad values back to their source.
4. **Log Evidence:** If the failure is in a distributed campaign, check `vox telemetry` or add temporary log instrumentation.

## Phase 2: Scientific Method

1. **Hypothesis:** "I think X is the root cause because Y."
2. **Test Hypothesis:** Make the smallest possible change or write a targeted diagnostic test.
3. **Verify:** Confirm if the hypothesis holds.

## Phase 3: Fix & Verify

1. **Failing Test:** Reproduce the bug in a TDD-compliant test.
2. **Implement Fix:** Fix at the source, not the symptom.
3. **Circuit Breaker:** If 3 fixes fail for the same issue, STOP. Summarize the failure and check if the architecture is flawed.

## Red Flags

- "Just trying this to see if it works."
- Fixing symptoms at call sites instead of root causes.
- Ignoring `vox stub-check` findings.
