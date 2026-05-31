---
title: "Rust via Vox — Lesson 0: Reading Rust Syntax"
description: "The syntax primer Lesson 1 assumed: let bindings, the & borrow symbol, String vs &str, macros, format placeholders, closures, and return types."
category: "Tutorials"
last_updated: "2026-05-28"
training_eligible: false
---

# Lesson 0 — Reading Rust Syntax (the primer Lesson 1 assumed)

> **Why this exists:** Lesson 1 jumped straight to ownership mechanics and assumed you could already read the syntax. You can't yet — and that's normal. This lesson teaches the handful of symbols and shapes you'll see in *every* Rust file, so the ownership lessons land. Nothing here is hard; it's vocabulary.

You only need to genuinely understand five things to read most Rust. We'll do each, then a tiny check.

---

## 1. `let` bindings — naming a value

```rust
let n = 42;            // n is bound to 42. Type inferred as i32.
let n: i32 = 42;       // same, but the type is written explicitly.
let n = 42i32;         // same again — the `i32` suffix tags the literal's type.
```

- `let name = value;` introduces a name.
- Bindings are **immutable by default** — you can't reassign `n` later unless you write `let mut n = 42;`.
- The `i32` in `42i32` is *not* generics and *not* angle brackets. It's just a type label glued to the number. `42`, `42i32`, and `42_i32` are identical.

**Number types you'll see:** `i32` (32-bit signed int — the default), `i64`, `u32` (unsigned), `u8` (a byte), `usize` (pointer-sized, used for lengths/indices), `f64` (float).

---

## 2. `&` — the borrow symbol

This is the most important symbol in Rust. **`&` means "a reference to" — a borrow.**

```rust
let n = 42;
let r = &n;     // r is a reference pointing AT n. r is not 42; r points to where 42 lives.
```

Compare:

```rust
let m = n;      // NO &. m is a copy (for i32) or a move (for String/Vec). m does NOT point at n.
let r = &n;     // HAS &. r is a reference. r points at n.
```

| You wrote | Meaning |
|-----------|---------|
| `n` | the value itself (copied or moved) |
| `&n` | a shared (read-only) reference to `n` |
| `&mut n` | a mutable (read+write) reference to `n` |

A function parameter `s: &String` means "I borrow a String to look at it." Calling it as `f(&name)` lends `name` without giving it away. **No `&` at the call site → the value is handed over (moved/copied) instead of lent.**

This single symbol is the whole game. When reading code, your eye should snap to every `&`.

---

## 3. `String` vs `&str` — owned text vs borrowed text

```rust
let owned: String = String::from("hello");   // owns heap memory, can grow
let view: &str = "hello";                    // a borrowed read-only window into text
let view2: &str = &owned;                    // borrow the String as a &str
```

- **`String`** = owns its letters. Allocated on the heap, responsible for freeing them, can grow with `.push_str(...)`. Like owning a notebook.
- **`&str`** = a borrowed view into text someone else owns. Read-only, fixed length, frees nothing. Like reading over a shoulder.
- A quoted literal `"hello"` is a `&str` baked into your program.
- `String::from("hello")` *copies* that literal into a fresh owned `String`.

Rule of thumb you'll use constantly in review: **if a function only reads text, it should take `&str`, not `String`** — so callers don't have to allocate.

---

## 4. Macros — the `!`

```rust
println!("hi");          // println! is a MACRO (note the !)
vec![1, 2, 3];           // vec! is a macro that builds a Vec
format!("{} {}", a, b);  // format! builds a String
```

The **`!` means "this is a macro, not a function."** A macro is code that writes code at compile time. You don't need to know how they work internally — just recognize that `name!(...)` or `name![...]` is a macro call. The three you'll see most:

- `println!` / `print!` — print to the screen.
- `vec!` — build a `Vec` (a growable, heap-allocated list — think "resizable array").
- `format!` — build a `String` from a template.

---

## 5. Format placeholders — `{}` and `{:?}`

Inside `println!` / `format!`, curly braces are fill-in slots:

```rust
println!("{}", n);        // {}   = "Display": human-friendly. Works for numbers, &str, String.
println!("{:?}", v);      // {:?} = "Debug": developer-friendly. Works for Vec, structs, etc.
println!("{} and {}", a, b);  // multiple slots, filled left to right
```

- Use `{}` for simple things (numbers, strings).
- Use `{:?}` for compound things (`Vec`, structs) where there's no obvious human format. `println!("{}", my_vec)` won't compile; `println!("{:?}", my_vec)` will.

---

## 6. Closures — `|x|` is an inline function, NOT absolute value

When you see pipes around a name, that's a **closure** — a small anonymous function written inline:

```rust
let add_one = |x| x + 1;        // given x, produce x + 1
add_one(5);                     // == 6
```

The `|x|` lists the closure's arguments; the part after is what it computes. The math meaning of `|...|` (absolute value) does **not** exist in Rust — pipes are always a closure here. You'll see them constantly inside iterator methods:

```rust
words.filter(|w| *w == target)  // for each w, keep it if it equals target
     .map(|n| n * 2)            // for each n, produce n * 2
```

`filter` and `map` each take a closure and run it on every element.

## 7. Functions and `->` return types

```rust
fn greet(name: &str) -> String {     // takes a &str, RETURNS a String
    format!("hello {name}")
}
```

- `fn name(params) -> ReturnType { body }`.
- The `->` introduces the **return type** — it is **not** a cast. `-> String` means "hands back a String"; `-> usize` means "hands back a count."
- The last expression in the body (with no semicolon) is the return value. `format!(...)` above is the returned String.
- If a function returns nothing, you omit `->` (it implicitly returns `()`, the empty "unit" type).

## That's the whole vocabulary

With those five, here's EX2 from Lesson 1, fully readable:

```rust
fn main() {
    let v = vec![1, 2, 3];      // vec! macro builds a Vec<i32>. v owns it.
    let w = v;                  // no & → and Vec isn't Copy → v is MOVED into w. v is now dead.
    println!("{:?}", v);        // {:?} debug-prints... but v was moved. COMPILE ERROR here.
}
```

Every symbol now has a name. That's the goal of this lesson.

---

## Check yourself (write answers in chat, one line each)

These are *recall* questions — produce the answer, don't pick from a list. "I don't know" is a fine answer; it tells me where to slow down.

**C1.** In `let x = &y;`, is `x` a copy of `y`, or a reference pointing at `y`? Which symbol tells you?

**C2.** What's the difference between `vec!` and `vec` — what does the `!` signify?

**C3.** A function is declared `fn greet(name: &str)`. Why is `&str` a better choice here than `String`?

**C4.** You try `println!("{}", scores)` where `scores` is a `Vec<i32>`, and it won't compile. What one-character change to the format string fixes it?

**C5.** In `let s = String::from("hi");`, does `s` own its text or borrow it? What about the `"hi"` part specifically?

**C6.** Predict: does this compile?
```rust
let a = String::from("x");
let b = a;
println!("{}", a);
```
If not, which line breaks and what's the rule? (This is the same shape as EX2 — apply it.)

---

Once these six feel solid, we go back to Lesson 1's EX1–EX3, and they'll read very differently. No rush — this primer is the foundation everything else stands on.
