# Example: Durable Execution Example

```vox
# Durable Execution Example
# Demonstrates activities, workflows, and the `with` expression syntax.

type OrderResult =
    | Ok(order_id: str)
    | Error(message: str)

type PaymentResult =
    | Ok(tx_id: str)
    | Error(message: str)

# Activities are units of work that may fail and be retried.
# They must return a Result type.
activity validate_order(order_data: str) to Result[str]:
    let validated = "validated-" + order_data
    ret Ok(validated)

activity charge_payment(amount: int, card_token: str) to Result[str]:
    let tx = "tx-" + card_token
    ret Ok(tx)

activity send_confirmation(recipient: str, order_id: str) to Result[str]:
    let msg = "Order " + order_id + " confirmed for " + recipient
    ret Ok(msg)

# Workflows orchestrate activities with durable execution guarantees.
# The `with` expression applies retry/timeout policies to activity calls.
workflow process_order(customer: str, order_data: str, amount: int) to Result[str]:
    # Validate with a short timeout and no retries
    let validated = validate_order(order_data) with { timeout: "5s" }

    # Charge payment with retries and backoff
    let payment = charge_payment(amount, "card-123") with { retries: 3, timeout: "30s", initial_backoff: "500ms" }

    # Send confirmation with basic retry
    let confirmation = send_confirmation(customer, "order-001") with { retries: 2, activity_id: "confirm-order-001" }

    ret confirmation
```
