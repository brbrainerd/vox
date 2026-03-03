# How-To: Test Your Logic

Learn how to write and run automated tests for your Vox application using the built-in test runner.

## 1. Writing Unit Tests

Use the `@test` decorator to mark functions as test cases. These functions can be run with the `vox test` command.

```vox
# Skip-Test
@test fn test_addition():
    assert(1 + 1 == 2)
```

## 2. Using Fixtures

Fixtures provide set-up logic that can be reused across multiple tests. Use the `@fixture` decorator.

```vox
# Skip-Test
@fixture fn mock_db() to Database:
    ret spawn MockDatabase()

@test fn test_query(db = mock_db):
    let result = db.call(query("SELECT 1"))
    assert(result == [1])
```

## 3. Mocking Dependencies

Use the `@mock` decorator to intercept calls to external services or server functions during testing.

```vox
# Skip-Test
@mock fn mock_email_service(to: str, msg: str):
    print("Mock sent: " + msg)

@test fn test_signup():
    with mock_email_service:
        signup("test@example.com")
```

## 4. Integration Testing

Test your full-stack logic by running the compiler and checking the generated output or simulating HTTP requests.

```bash
# Run all tests in the project
vox test src/
```

---

**Related Reference**:
- [CLI Reference](api/vox-cli.md) — `vox test` flags and configuration.
- [Tutorial: First App](tut-first-app.md) — Example of testing a todo list.
