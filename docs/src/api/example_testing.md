# Example: TESTING

```vox
type Res =
    | Success(v: str)
    | Error

@test fn test_addition() to Unit:
    let sum = 1 + 2
    assert(sum is 3)

@test fn test_strings() to Unit:
    let s = "foo"
    let s2 = "bar"
    let res = s + s2
    assert(res is "foobar")

@test fn test_success() to Unit:
    let r = Success("ok")
    match r:
        Success(v) -> assert(v is "ok")
        Error -> assert(false)

@test fn test_str_cast() to Unit:
    let n = 42
    let s = str(n)
    assert(s is "42")
```
