---
title: "How-To: Test Your Logic"
description: "Write automated test cases using Vox."
category: "How-To Guides"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---
# How-To: Test Your Logic

Learn how to write and run automated tests for your Vox application using the built-in test runner.

## 1. Writing Unit Tests

Use the `@test` decorator to mark functions as test cases. These functions can be run with the `vox test` command.

```vox
// vox:skip
@test 
fn test_addition() to Unit {
    assert(1 + 1 == 2)
}
```

## 2. Hand-Rolled Setup Helpers (Fixtures)

Rather than language-level magic, Vox encourages simple, plain functions for setup logic that can be reused across test cases.

```vox
// vox:skip
fn setup_mock_db() to Database {
    return spawn MockDatabase()
}

@test 
fn test_query() to Unit {
    let db = setup_mock_db()
    let result = db.call(query("SELECT 1"))
    assert(result == [1])
}
```

> [!WARNING]
> Historical decorators `@fixture` and `@mock` are considered aspirational. Use standard helper functions for state-setup instead.

## 3. Property Writing with `@forall`

Vox supports property-based testing. The test runner will generate random inputs for your function to find edge cases where your assertions fail.

```vox
// vox:skip
@forall
fn test_addition_commutative(a: int, b: int) to Unit {
    assert(a + b == b + a)
}
```

## 4. Fuzzing with `@fuzz`

For deeper security and stability testing, the `@fuzz` decorator uses the project's native LLVM-based fuzzer to explore illegal execution paths.

```vox
// vox:skip
@fuzz
fn fuzz_parser(input: str) to Unit {
    let _ = parse_json(input) // Fuzzer tries to crash this
}
```

## 5. Running Tests and Output Format

Use the `vox test` command to execute your suite.

```bash
vox test src/
```

**Output Example**:
```text
[PASS] tests::test_addition (1.2ms)
[PASS] tests::test_addition_commutative (100 iterations)
[FAIL] tests::fuzz_parser
       > Reason: Panic at core.vox:120 (division by zero)
       > Input: "{"a": 0}"
```

## Summary
- Use `@test` for standard unit tests.
- Use `@forall` for property-based data validation.
- Use `@fuzz` for security and crash-resilience testing.
- Write standard functions that serve as setups, fixtures, and mocks explicitly.
- Run `vox test <path>` to execute blocks tagged with `@test`.

## Related
- [CLI Reference](../reference/cli.md) — `vox test` flags and configuration.
- [Durable Workflows](../tutorials/tut-workflow-durability.md) — Understanding testable workflows.
