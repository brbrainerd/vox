---
title: "How-To: Model Complex Domain Logic"
description: "Learn how to use Vox's expressive type system."
category: "How-To Guides"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---
# How-To: Model Complex Domain Logic

Learn how to use Vox's expressive type system to model your application's domain logic effectively.

## 1. Algebraic Data Types (ADTs)

Vox supports powerful ADTs (sum types) for representing state that can be one of several variants.

```vox
// vox:skip
type OrderStatus =
    | Pending
    | Processing(staff_id: str)
    | Shipped(tracking_number: str)
    | Delivered(timestamp: int)
```

## 2. Pattern Matching

Use the `match` expression to handle ADT variants with full type safety.

```vox
// vox:skip
fn describe_status(status: OrderStatus) to str {
    return match status {
        Pending         -> "Waiting for staff"
        Processing(id)  -> "Being handled by " + id
        Shipped(track)  -> "In transit { " + track
        Delivered(_)    -> "Package reached destination"
    }
}
```

## 3. Composing Structs

Group related data into named structs.

```vox
// vox:skip
type Address {
    street: str
    city:   str
    zip:    int
}

type Customer {
    name:  str
    email: str
    shipping_address: Address
}
```

## 4. Validation with `@require`

Add runtime guards to your data types using the `@require` decorator.

```vox
// vox:skip
@require(len(self.password) > 8)
type UserAccount {
    username: str
    password: str
}
```

## Summary
- Describe mutually exclusive states and data variants cleanly using ADTs (Sum Types).
- Avoid invalid states with constructor validation guards via `@require`.
- Pattern match to strictly process all possibilities at compile time.

## Related
- [Language Syntax](../reference/ref-syntax.md) — Full type system syntax.
- [Database Schema](../reference/ref-db-surface.md) — Modeling domain with tables.
