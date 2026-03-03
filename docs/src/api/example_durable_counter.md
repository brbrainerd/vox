# Example: Vox Durable Actor Demo

```vox
# Vox Durable Actor Demo
# Demonstrates persistent state via `state_load` and `state_save`

import react.use_state

@component fn CounterApp() to Element:
    let (count, set_count) = use_state(0)

    let increment = fn (e) set_count(spawn(PersistentCounter).increment())

    <div class="counter_container">
        <h1>"Persistent Counter"</h1>
        <p>"Current Value: " {count}</p>
        <button on_click={increment}>"Increment"</button>
    </div>

style:
    .counter_container:
        fontFamily: "sans-serif"
        display: "flex"
        flexDirection: "column"
        alignItems: "center"
        padding: "2rem"

routes:
    "/" to CounterApp

actor PersistentCounter:
    on increment() to int:
        let current = state_load("counter")
        let next = current + 1
        state_save("counter", next)
        ret next
```
