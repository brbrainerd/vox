# Reference: Type System

Vox features a strongly-typed, expressive type system designed for technical unification between Rust (backend) and TypeScript (frontend).

## 1. Primitive Types

| Type | Description | Rust Equivalent | TS Equivalent |
|------|-------------|-----------------|---------------|
| `str` | UTF-8 String | `String` | `string` |
| `int` | 64-bit Integer | `i64` | `number` (or `BigInt`) |
| `float`| 64-bit Float | `f64` | `number` |
| `bool` | Boolean | `bool` | `boolean` |

## 2. Collection Types

### `list[T]`
An ordered sequence of elements of type `T`.
- **Usage**: `list[int]`, `list[User]`
- **Rust**: `Vec<T>`
- **TS**: `T[]`

### `map[K, V]`
A collection of key-value pairs.
- **Usage**: `map[str, int]`
- **Rust**: `HashMap<K, V>`
- **TS**: `Record<K, V>`

## 3. Algebraic Data Types (ADTs)

### Structs
Named collection of fields.
```vox
# Skip-Test
type Point:
    x: int
    y: int
```

### Enums (Sum Types)
Types that can be one of several variants.
```vox
# Skip-Test
type Shape:
    | Circle(radius: float)
    | Square(side: float)
```

## 4. Special Types

### `Result[T, E]`
Represents either success (`Ok(T)`) or failure (`Err(E)`).
- **Usage**: `Result[User, str]`
- **Propogation**: Use the `?` operator.

### `Option[T]`
Represents an optional value.
- **Usage**: `Option[str]`
- **Variants**: `Some(T)` or `None`.

## 5. Inference & Subtyping
- **Global Inference**: Vox can often infer types for local variables.
- **Structural Subtyping**: (Coming soon) Components may admit structural compatibility in certain UI contexts.
