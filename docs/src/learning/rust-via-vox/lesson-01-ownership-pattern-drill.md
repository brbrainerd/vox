---
title: "Lesson 1 — Ownership in Practice: Reading Code Like a Reviewer"
description: "Rust-via-Vox training lesson: recognize ownership mechanics (Copy/move/borrow/drop) in real function signatures, recursive types, and AI-generated code fast enough to do meaningful code review."
category: "Tutorials"
last_updated: "2026-05-30"
training_eligible: false

schema_type: "TechArticle"
---

# Lesson 1 — Ownership in Practice: Reading Code Like a Reviewer

> **Goal:** recognize ownership mechanics in real code fast enough to do meaningful code review. The atoms (Copy/move/borrow/drop) are already in your head — this lesson trains *transfer*: applying them to function signatures, recursive types, and AI-generated production code without losing track of who owns what.

## What this lesson is fixing (from your diagnostic)

Two coherent gaps emerged from 44 diagnostic questions:

1. **`Result<T, E>` / `Option<T>` / `into_iter` look like they wrap borrows when they actually carry owned values.** You confidently said `fn parse(s: &str) -> Result<Config, E>` returns a borrowed Config tied to `s`. It doesn't. The borrowed input and the owned output are independent. Same gap caused the `into_iter` miss.
2. **`drop(x)` reads like "schedule cleanup later" but it's an immediate by-value move into the `drop` function.** The borrow checker enforces it exactly like any other by-value call.

This lesson drills both directly.

## Why this matters for the Rust job market (2026)

Rust hiring in 2026 is increasingly review-shaped: someone has to look at what Copilot/Claude/Cursor produced and decide what's safe to merge. AI-generated Rust passes the compiler but fails review in predictable ways — and *you* coming from a heavily-AI-assisted writing background gives you a sharp angle on this work, because you've *been* the person whose code needed review.

The repeat offenders that show up across web (axum/actix), CLI (clap/anyhow), compiler/database (where Vox lives), and async-server work:

| AI mistake | What a reviewer says |
|------------|---------------------|
| `.clone()` everywhere to silence the borrow checker | "Restructure to borrow; clone has a runtime cost" |
| `fn foo(s: String)` when only reading | "Take `&str`; callers shouldn't have to allocate" |
| `Arc<Mutex<T>>` in single-threaded code | "Just use `&mut self`" |
| `Rc<RefCell<T>>` in multi-threaded code | "That's not `Send`; will it cross threads?" |
| `unwrap()` on user/network input | "DoS vector — propagate via `?`" |
| `Box<dyn Trait>` where `impl Trait` works | "Pay a vtable cost; can this be monomorphized?" |
| Holding a `MutexGuard` across `.await` | "Deadlock; drop the guard before awaiting" |
| `x.len() == 0` instead of `x.is_empty()` | "std has the named method; use it" |
| `'a` annotations where elision works | "Compiler infers these; delete them" |
| `format!("{}{}", a, b)` for two-string concat | "Use `&a + b` or `push_str`; format! is a heavyweight" |

Lessons 1-4 build the *ownership mechanics* that let you see these. Lessons 5-8 turn them into review reflexes. This lesson is the foundation: if you can't trace who owns what in a function, you can't argue about whether a clone is necessary.

## Concept recap (60 seconds)

When you write `let y = x;` or pass `x` to a function, one of three things happens:

| What | When | Aftermath |
|------|------|-----------|
| **Copy** | `x`'s type implements the `Copy` trait | Both `x` and `y` usable. Cheap (bit-level duplicate). |
| **Move** | `x`'s type does *not* implement `Copy` | `x` invalidated. `y` is sole owner. |
| **Borrow** | You wrote `&x` or `&mut x`, not `x` | `x` keeps ownership; `y` is a reference under the borrow rule. |

**Rule of thumb for `Copy`:** if the value is entirely on the stack and contains no owned heap resource, it can be `Copy` — `i32`, `bool`, `char`, `f64`, `(i32, bool)`, `[u8; 4]`. **`String`, `Vec<T>`, `Box<T>`, `HashMap`** are *not* `Copy` because each owns a heap allocation; copying would create two owners of one buffer.

**Two corrections from your diagnostic:**
- `Result<T, E>::Ok(t)` carries an *owned* `t`. The wrapper is irrelevant to ownership — what matters is whether `T` itself is owned or a reference. `Result<Config, E>` returns an owned `Config`; `Result<&Config, E>` returns a borrow. Read the inner type, not the wrapper.
- `drop(x)` is `fn drop<T>(_: T)` — it takes `x` by value and lets it fall out of scope inside the function. Identical to writing `let _ = x;` (almost). It's not a hint to the runtime; it's a move that happens *right now*.

## Exercise format

Each exercise is one of:

- **MECHANICS** — short snippet, predict compile/move/borrow and explain *why* in one sentence.
- **REVIEW** — AI-style production code that compiles and runs. Your job is to find what a reviewer would flag and propose the fix.

"What" is easy ("it errors"). "**Why**" is the skill — name the concept (move, borrow rule, drop semantics, signature smell). One sentence is plenty.

Don't peek at later exercises until you've answered the current one.

---

### EX1 — MECHANICS (Copy types)

```rust
fn main() {
    let n = 42i32;
    let m = n;
    println!("{} {}", n, m);
}
```

**Q1:** Does this compile? After `let m = n;`, what's the relationship between `n` and `m`?

---

### EX2 — MECHANICS (move via assignment)

```rust
fn main() {
    let v = vec![1, 2, 3];
    let w = v;
    println!("{:?}", v);
}
```

**Q2:** Does this compile? If not, which line errors and what's the rule being broken?

---

### EX3 — MECHANICS (shared borrow across function call)

```rust
fn print_len(s: &String) {
    println!("len = {}", s.len());
}

fn main() {
    let name = String::from("Beth");
    print_len(&name);
    println!("still have name: {}", name);
}
```

**Q3:** Does this compile? What is the `&` in `print_len(&name)` doing — at the level of ownership, not just syntax?

---

### EX4 — MECHANICS (borrow rule under stress)

```rust
fn main() {
    let mut v = vec![1, 2, 3];
    let first = &v[0];
    v.push(4);
    println!("{}", first);
}
```

**Q4:** Does this compile? Walk through which borrows are live at each line. If you wanted to both push *and* print `first`, what's the minimal restructuring?

---

### EX5 — MECHANICS (the `drop` correction)

```rust
fn main() {
    let s = String::from("hi");
    let r = &s;
    drop(s);
    println!("{}", r);
}
```

**Q5:** Does this compile? Explain at the type level what `drop(s)` is doing — pretend you're explaining to someone who thinks `drop` schedules cleanup for the end of scope.

---

### EX6 — REVIEW (over-cloning, AI flavor)

```rust
fn count_word(text: String, word: String) -> usize {
    text.split_whitespace()
        .filter(|w| w.to_string() == word.clone())
        .count()
}

fn main() {
    let body = String::from("the quick brown fox the lazy fox");
    let target = String::from("fox");
    println!("{}", count_word(body.clone(), target.clone()));
    println!("body still alive: {}", body);
}
```

**Q6:** This compiles and runs. Name **three** review issues (signatures + body + call site) and rewrite `count_word`'s signature. Hint: this is the most common shape of AI-generated Rust — clone-cascades to silence the borrow checker.

---

### EX7 — REVIEW (`Arc<Mutex>` cargo-culted)

```rust
use std::sync::{Arc, Mutex};

struct Counter {
    value: Arc<Mutex<u32>>,
}

impl Counter {
    fn new() -> Self {
        Counter { value: Arc::new(Mutex::new(0)) }
    }
    fn increment(&self) {
        let mut v = self.value.lock().unwrap();
        *v += 1;
    }
    fn get(&self) -> u32 {
        *self.value.lock().unwrap()
    }
}

fn main() {
    let c = Counter::new();
    c.increment();
    c.increment();
    println!("{}", c.get());
}
```

**Q7:** Single-threaded use (main calls everything sequentially). What does a reviewer flag? What's the minimum-viable `Counter` that does the same job? Hint: `&mut self` will appear.

---

### EX8 — REVIEW (signature smell + `unwrap`)

```rust
fn read_config(path: String) -> String {
    let bytes = std::fs::read(path).unwrap();
    String::from_utf8(bytes).unwrap()
}
```

**Q8:** Two issues at the boundary (signature), two on the body. Write the corrected signature *and* the body. The signature should accept things callers can pass without allocating, and the function should propagate errors instead of panicking.

---

### EX9 — MIXED (`into_iter` vs `iter` — direct hit on your diagnostic miss)

```rust
fn sum_pairs(v: Vec<(String, i32)>) -> i32 {
    let names: Vec<String> = v.iter().map(|(n, _)| n.clone()).collect();
    let total: i32 = v.into_iter().map(|(_, n)| n).sum();
    println!("processed {:?}", names);
    total
}
```

**Q9:** This compiles. (a) State what happens to `v` on each of the two iterator lines. (b) Could the second line use `v.iter()` instead — yes/no, why? (c) Is the `clone()` on the first line avoidable, and at what cost?

---

### EX10 — REVIEW (Vox-shaped: recursive AST with `Box`)

> The shape below mirrors a tiny slice of [`crates/vox-compiler/src/ast/expr.rs`](../../../../crates/vox-compiler/src/ast/expr.rs) — recursive AST nodes hidden behind `Box` so the enum has a known size. You don't need to know Vox to answer; this exercise is about *recursion over owned trees*, which shows up in every compiler, parser, JSON library, and game-engine scene graph.

```rust
enum Expr {
    Lit(i64),
    Add(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
}

fn depth(e: Expr) -> usize {
    match e {
        Expr::Lit(_) => 1,
        Expr::Add(l, r) => 1 + std::cmp::max(depth(*l), depth(*r)),
        Expr::Neg(inner) => 1 + depth(*inner),
    }
}

fn main() {
    let e = Expr::Add(
        Box::new(Expr::Lit(1)),
        Box::new(Expr::Neg(Box::new(Expr::Lit(2)))),
    );
    let d1 = depth(e);
    let d2 = depth(e);  // <-- error here
    println!("{} {}", d1, d2);
}
```

**Q10:** (a) Why does the second `depth(e)` fail — what's the signature of `depth` doing? (b) Change `depth`'s signature so `main` works as written, *without* adding `Clone`. (c) A junior reviewer says "just `#[derive(Clone)]` and call `e.clone()`." When is that defensible and when is it sloppy? Think about what a 10,000-node AST would cost.

---

## Bonus stretch — if you breeze through 1-10

### EX11 — REVIEW (the `MutexGuard` across `.await` trap, async preview)

```rust
async fn handle(state: &Mutex<u64>) {
    let mut guard = state.lock().unwrap();
    *guard += 1;
    some_async_work().await;
    *guard += 1;
}
# async fn some_async_work() {}
```

**Q11:** What's the bug? Why is this a *deadlock* and not just a performance issue? This pattern shows up constantly in AI-generated async Rust — it compiles fine on a single-threaded runtime and explodes on a multi-threaded one.

---

## Passing this lesson

You don't need 10/10. You need to **answer each one with a one-sentence reason that names the concept** (Copy, move, borrow rule, drop semantics, signature smell, single-vs-multi-threaded, unsized-needs-Box, etc.). If ~8 of your reasons are sharp, Lesson 2 unlocks. If ~4+ are vague ("the borrow checker doesn't like it"), we re-drill before moving on — a wobbly Lesson 1 makes Lessons 3 (lifetimes) and 9 (trait objects) collapse.

The REVIEW exercises (EX6, 7, 8, 10, 11) matter most for the job-market angle. Those are the conversations you'd have on a PR.

---

## Next lesson preview

**Lesson 2 — Iterator ownership applied.** You'll derive `iter` / `iter_mut` / `into_iter` from the rules you just drilled, so `into_iter` consuming `v` becomes inevitable rather than memorized. Includes an extended REVIEW where you fix a 40-line AI-generated transform pipeline that clones at every stage.
