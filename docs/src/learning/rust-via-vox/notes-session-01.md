---
title: "Rust via Vox — Session 1 Notes: Ownership, Errors, Enums"
description: "Consolidated revision notes from session 1: move/borrow/copy, references and &mut self, the ? operator and Result, iter vs into_iter, enums and match-as-expression, format!, .into(), and the review reflexes built from each."
category: "Tutorials"
last_updated: "2026-05-28"
training_eligible: false
---

# Session 1 Notes — Ownership, Errors, Enums

> Revision notes for everything covered in the first working session. Each concept includes the rule, a tiny example, and **the review reflex** it unlocks — because the goal is reviewing AI-generated Rust, not just reading it. Misconceptions that came up are marked ⚠️ so you can re-check yourself.

## The one idea everything hangs on

> **Borrow by default. Only move, own, or clone when you actually need to.**

Every review reflex below is a special case of this. AI-generated Rust violates it constantly (it clones and takes ownership to silence the borrow checker), which is exactly what makes it reviewable.

---

## 1. Copy vs Move vs Borrow

When you write `let y = x;` or pass `x` to a function, one of three things happens:

| | When | Aftermath |
|---|---|---|
| **Copy** | `x`'s type implements `Copy` (small stack types: `i32`, `bool`, `char`, `f64`) | both `x` and `y` usable |
| **Move** | `x`'s type owns heap data and is **not** `Copy` (`String`, `Vec`, `Box`) | `x` is **dead**; `y` is sole owner |
| **Borrow** | you wrote `&x` or `&mut x` | `x` keeps ownership; `y` is a reference |

```rust
let a = 5;     let b = a;     // i32 is Copy → both alive
let s = String::from("x"); let t = s;   // String moves → s is DEAD
let r = &s;    // & makes a reference → s stays alive, r points at it
```

⚠️ **My early mistake:** I thought `let m = n;` made `m` a *reference* to `n`. It doesn't — **no `&` means no reference.** Without `&` it's a copy (Copy types) or a move (everything else).

**Rule of thumb for Copy:** entirely on the stack, no owned heap resource → can be Copy. Anything owning a heap allocation (String, Vec, HashMap, Box) is **not** Copy, because copying would create two owners of one buffer.

**Passing to a function = a hidden `let`.** `show(data)` is really `let v = data` at the boundary. No `&`, non-Copy → the argument moves and the caller's binding dies.

**Review reflex:** see a `.clone()` → ask *"is this necessary, or is it dodging a borrow? Could this be `&`?"* Cloning duplicates the whole heap allocation — fine for a 3-element Vec, a disaster for a 10-million-element one.

---

## 2. The borrow rule

At any moment, for a given value, you may have **either**:
- any number of shared references `&T` (read-only), **or**
- exactly one mutable reference `&mut T` (read+write).

Never both at once. This is what prevents data races at compile time.

---

## 3. Stack vs Heap (and why ownership exists)

| | Stack | Heap |
|---|---|---|
| size | fixed, known at compile time | decided at runtime, **can grow** |
| speed | very fast | slower (managed) |
| cleanup | automatic on function return | must be tracked → **this is what ownership is for** |
| examples | `i32`, `bool`, a reference | the text in a `String`, the elements of a `Vec` |

Heap memory must be freed or you leak it. Three approaches: C (manual `free` — error-prone), Java/Go (garbage collector — runtime cost), **Rust (ownership — each allocation has one owner; freed automatically when the owner goes out of scope, no GC).** The move rule guarantees exactly one owner so nothing is freed twice. **The borrow checker is just Rust enforcing "one owner, no dangling references" at compile time.**

---

## 4. References in signatures & `&mut self`

- `fn f(s: &str)` — borrows to **read**. Callers don't allocate.
- `fn f(s: &mut String)` — borrows to **modify** the caller's String.
- `fn f(s: String)` — **takes ownership**; caller's binding dies.

For methods, `self` works the same way:
- `&self` — borrows the object to read it.
- `&mut self` — borrows it to modify its fields.
- `self` — consumes it.

**Review reflex (over-engineering):** `Arc<Mutex<u32>>` + `&self` is often an elaborate disguise for `u32` + `&mut self`. `Arc<Mutex<T>>` only earns its keep when data is **actually shared across threads**. Single-threaded? Ask why it's there.

⚠️ **Interior mutability:** `Mutex` and `RefCell` let you mutate *through* a shared `&self` borrow — that trick is called interior mutability. Remove the `Mutex` and the honesty moves to the signature: you now need `&mut self`.

**Signature + call site move together:** you can't borrow at the call (`f(&x)`) unless the signature agrees to accept a borrow (`f(s: &str)`). Change one, change the other.

---

## 5. `String` vs `&str`, and friends

- **`String`** owns heap text, can grow, frees itself.
- **`&str`** is a borrowed read-only view; a literal `"hi"` is a `&str`.
- `String::from("hi")` copies the literal into an owned String. `"hi".to_string()` does the same thing (identical result).
- **Deref coercion:** `&String` is accepted anywhere a `&str` is wanted — the conversion is automatic, you write nothing.

**Review reflex:** a function that only reads text should take `&str` (or `&Path` for paths), not `String`. Taking `String` forces callers to allocate.

---

## 6. Error handling: `unwrap`, `?`, `Result`, `Ok`

- **`Result<T, E>`** is `Ok(value)` or `Err(problem)`.
- **`.unwrap()`** means *"give me the Ok value, or **crash the whole program** if it's an Err."* Turning a recoverable problem (missing file, bad input) into a crash is the sin.
- **`?`** means *"unwrap the Ok, or **return the Err from this function right now**."* It's an **early return**.
- **`?` requires the function to return `Result`** (or `Option`) — there has to be a slot for the error to go into.
- **`?` unwraps (on the way in); `Ok(...)` wraps (on the way out).**

```rust
fn read_config(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;        // Err → returns here, skips the rest
    let text  = String::from_utf8(bytes)?;   // bytes is a plain Vec<u8> (already unwrapped)
    Ok(text)                                  // success must be re-wrapped to match the signature
}
```

⚠️ **`?` on an Err is an immediate `return`.** Everything below it is skipped — including the final `Ok`. When the first `?` fails, `bytes` simply never gets created, and **that's fine, not an error** (same as any `return` — later code just doesn't run). Only *one* `?` ever fires per failure: the first to hit an `Err`.

**Do you need an outer `Ok(...)`?** Ask: *is the value I'm returning already a `Result`, or a plain value?*
- plain value at the end → wrap it: `Ok(value)` (e.g. `text` above).
- already a `Result` → return as-is (e.g. a `match` whose arms each produce `Ok`/`Err`).

**Review reflex:** `.unwrap()` on user/file/network input is a denial-of-service — anyone can force a crash. Should be `?` into an error type the caller handles.

---

## 7. `iter()` vs `into_iter()`

- **`for w in v.iter()`** — *borrows* each element. `v` survives the loop. Each `w` is a `&T`.
- **`for w in v.into_iter()`** — *consumes* `v`. Each element moves out; `v` is **dead** afterward. Each `w` is an owned `T`.

Same move-vs-borrow rule, applied to loops. `into_iter` = "no `&`, take by value" = move.

```rust
for w in words.iter() { /* read w */ }
println!("{}", words.len());   // works — iter() only borrowed
```

You can call methods (`.len()`, etc.) straight through a reference, so borrowing usually costs nothing.

**Review reflex:** `into_iter()` where the collection is used afterward is a bug; `into_iter()` where `iter()` would do is needless ownership.

---

## 8. Enums and `match`

An **`enum`** is a type that is **exactly one of several variants**, and variants can carry data:

```rust
enum Status {
    Active,           // carries nothing
    Pending(u32),     // carries a number
    Closed(String),   // carries a string
}

let s = Status::Pending(5);   // :: reaches into Status for the variant
```

**`match` is an expression, not a sequence of steps:**
- every arm is `Pattern => value`;
- **every arm must produce the same type**;
- that type is the function's return value (the `match` *is* the return).

```rust
fn check_pr(s: Status) -> Result<String, Box<dyn std::error::Error>> {
    match s {
        Status::Active         => Ok("active".to_string()),
        Status::Pending(days)  => Ok(format!("pending for {days} days")),
        Status::Closed(reason) => Err(reason.into()),
    }
}
```

- The carried data comes out in the **pattern**: `Pending(days)` binds the number to `days`.
- No floating `Ok(...)` after the match — each arm already produces a full `Result`.
- `match` is **exhaustive**: forget a variant and it won't compile. (A big deal for review — you can't silently miss a case.)

⚠️ You can't put `let x = ...` in an arm (that's a statement, not a value), and you can't `?` a bare `bool` (`?` is for `Result`/`Option`). An arm can be an inline `if/else` **as long as both branches produce the right type.**

**Whether `Closed` is `Ok` or `Err` is a domain design decision, not a Rust rule** — questioning it ("should this really be an error?") is a legitimate review comment.

---

## 9. Two small builders

- **`format!("... {x} ...")`** — like `println!` but **returns a `String`** instead of printing. Stitches values into text.
- **`.into()`** — converts a value into whatever type is expected at that spot (the `From`/`Into` machinery). `Err(reason.into())` converts a `String` into the `Box<dyn Error>` the error slot wants.

---

## Review reflexes, collected

The job (reviewing AI-generated Rust) is mostly spotting violations of "borrow by default":

1. **Unnecessary `.clone()`** → "can this be `&`?"
2. **`String` param when only reading** → "take `&str`."
3. **`Arc<Mutex<T>>` with no cross-thread use** → "single-threaded? use `&mut self`."
4. **`.unwrap()` on external input** → "DoS — propagate with `?`."
5. **`into_iter()` when borrowing works** → "needless ownership."
6. **Signature and call site disagree** → "borrowing is a contract both sides sign."
7. **Forgotten error case / wrong `Ok` vs `Err` design** → exhaustive `match` + domain judgment.

---

## Concepts still ahead

- **`Box` and recursive types** (the deferred half of EX10 — why a recursive enum needs `Box`).
- **Lifetimes** (`'a`) — start by deleting the "fresh unrelated lifetime" model.
- **Trait objects vs generics** (`dyn` vs `impl` vs `<T: Trait>`).
- **Smart pointers in context** (`Rc`/`Arc`/`RefCell`/`Mutex`, `Send`/`Sync`).
- **Modern syntax** (`let-else`, `From`→`Into` blanket impl).
