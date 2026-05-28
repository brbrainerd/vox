---
name: test-driven-development
description: Use when implementing any feature or bugfix, before writing implementation code
---

# Test-Driven Development (Vox Adaptation)

Write the failing test first. Watch it fail. Write minimal code to pass.

**The Iron Law:** NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST.

## Red-Green-Refactor Cycle

1. **RED: Write Failing Test**
   - Identify the minimal behavior change.
   - Use `<thought>` to plan the test implementation.
   - For Rust: Create a `#[test]` in the same file or a new integration test in `tests/`.
   - For Vox: Create an `@test` block in the `.vox` file.

2. **Verify RED: Watch It Fail**
   - Run the test: `cargo test --package <crate> --lib <module>` or `vox run <file>.vox`.
   - **MANDATORY:** Verify the failure is due to missing logic, not a syntax error.

3. **GREEN: Minimal Code**
   - Write the simplest code to make the test pass.
   - NO PLACEHOLDERS: `todo!()` or `unimplemented!()` are architectural regressions.

4. **Verify GREEN: Watch It Pass**
   - Re-run the tests. Ensure ALL tests pass.
   - Run `vox stub-check` to ensure no new TOESTUB warnings.

5. **REFACTOR**
   - Clean up implementation while keeping tests green.

## Vox-Specific Rules

- Every new `pub fn` MUST have a test (enforced by `tdd-guard`).
- Use `vox_secrets` for all secret access even in tests.
- If a test is hard to write, the design is likely too coupled. Refactor the interface.
