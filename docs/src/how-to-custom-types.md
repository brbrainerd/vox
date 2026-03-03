# How-To: Model Complex Domain Logic

Learn how to use Vox's expressive type system to model your application's domain logic effectively.

## 1. Algebraic Data Types (ADTs)

Vox supports powerful ADTs (sum types) for representing state that can be one of several variants.

```vox
# Skip-Test
type OrderStatus:
    | Pending
    | Processing(staff_id: str)
    | Shipped(tracking_number: str)
    | Delivered(timestamp: int)
```

## 2. Pattern Matching

Use the `match` expression to handle ADT variants with full type safety.

```vox
# Skip-Test
fn describe_status(status: OrderStatus) to str:
    ret match status:
        | Pending => "Waiting for staff"
        | Processing(id) => "Being handled by " + id
        | Shipped(track) => "In transit: " + track
        | Delivered(_) => "Package reached destination"
```

## 3. Composing Structs

Group related data into named structs.

```vox
# Skip-Test
type Address:
    street: str
    city: str
    zip: int

type Customer:
    name: str
    email: str
    shipping_address: Address
```

## 4. Validation with `@require`

Add runtime guards to your data types using the `@require` decorator.

```vox
# Skip-Test
@require(len(self.password) > 8)
type UserAccount:
    username: str
    password: str
```

## 5. Summary

Modeling in Vox gives you:
- **Exhaustiveness Checking**: The compiler ensures you handle all variants.
- **Safety**: Prevent invalid states by construction.
- **Readability**: Clear architecture that maps directly to your business domain.

---

**Related Reference**:
- [Language Reference](ref-language.md) — Full type system syntax.
- [Architecture Explanation](expl-architecture.md) — How types flow through the HIR.
