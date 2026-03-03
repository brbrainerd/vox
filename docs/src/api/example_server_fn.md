# Example: Server function demo

```vox
# Server function demo
# Shows how @server generates both API route + typed fetch wrapper

type Greeting =
    | Hello(message: str)

@server fn greet(name: str) to Greeting:
    ret Hello("Welcome, " + name + "!")

@server fn add(a: int, b: int) to int:
    ret a + b
```
