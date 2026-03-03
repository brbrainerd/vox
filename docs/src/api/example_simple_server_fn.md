# Example: Simple server function demo

```vox
# Simple server function demo
# Shows @server generating both backend route and frontend client

type Greeting =
    | Hello(message: str)

@server fn greet(name: str) to Greeting:
    ret Hello("Welcome, " + name + "!")

@server fn add(a: int, b: int) to int:
    ret a + b

@component fn App() to Element:
    let result = use_state("")
    let handle_click = fn(_e):
        let greeting = greet("Alice")
        set_result(greeting)
    ret <div><button onClick={handle_click}>Greet</button><p>{result}</p></div>
```
