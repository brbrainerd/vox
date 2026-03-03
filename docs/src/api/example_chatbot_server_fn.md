# Example: Full-stack chatbot using server functions

```vox
# Full-stack chatbot using server functions
# Frontend component calls backend via auto-generated API

type Message =
    | User(text: str)
    | Assistant(text: str)

@server fn chat(prompt: str) to str:
    # In a real app, this would call an LLM API
    ret "Echo: " + prompt

@component fn Chatbot() to Element:
    let messages = use_state([])
    let input = use_state("")

    let send_message = fn(_e):
        # Call the server function (auto-generated fetch wrapper)
        let response = chat(input)
        let new_messages = messages.append(User(input)).append(Assistant(response))
        set_messages(new_messages)
        set_input("")

    ret <div className="chat-container">
        <h1>Vox Chatbot</h1>
        <div className="messages">
            {messages.map(fn(msg) match msg:
                | User(text) -> <div className="user-message">{text}</div>
                | Assistant(text) -> <div className="assistant-message">{text}</div>
            )}
        </div>
        <div className="input-area">
            <input
                value={input}
                onChange={fn(e) set_input(e.value)}
                placeholder="Type a message..."
            />
            <button onClick={send_message}>Send</button>
        </div>
    </div>
```
