# Example: Vox Actor Example

```vox
# Vox Actor Example
# Demonstrates the actor model: spawn, send/receive, and internal state.
#
# Actors are lightweight processes with isolated state and a mailbox.
# They communicate exclusively via message passing.

# Import statement (standard library modules)
import std.io

# Define a message type used between actors
message Greeting:
    from_name: str
    text: str

# A counter actor that tracks how many messages it has received.
#
# `state` fields hold the actor's mutable internal data.
# `on` handlers define how the actor responds to messages.
actor Counter:
    state count: int = 0

    # Receiving a plain string increments the counter
    on increment(amount: int) to int:
        count = count + amount
        count

    on get_count() to int:
        count

    # Reset the counter back to zero
    on reset() to Unit:
        count = 0

# A greeter actor that responds with a formatted greeting.
actor Greeter:
    on greet(name: str) to str:
        "Hello, " + name + "! Welcome to Vox."

# Main function demonstrating actor usage
fn main():
    # `spawn` creates a new actor instance and returns a handle (Pid)
    let counter = spawn(Counter)
    let greeter = spawn(Greeter)

    # `.send()` dispatches a message to the actor's mailbox
    let new_count = counter.send(increment(5))
    let greeting  = greeter.send(greet("Alice"))

    # Actors can be sent multiple messages
    let _ = counter.send(increment(3))
    let total = counter.send(get_count())   # returns 8

    ret total
```
