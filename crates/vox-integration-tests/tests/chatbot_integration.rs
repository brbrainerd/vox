use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::diagnostics::Severity;
use vox_typeck::typecheck_module;

fn check(src: &str) -> Vec<vox_typeck::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("Source should parse without errors");
    typecheck_module(&module)
}

fn errors(src: &str) -> Vec<vox_typeck::Diagnostic> {
    check(src)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

#[test]
fn chatbot_full_stack_integration() {
    // This example mimics chatbot.vox but adapted slightly for current parser
    // e.g. using string literals in JSX text nodes if parser needs it
    let src = r#"
type ChatResult =
    | Success(text: str)
    | Error(message: str)

@component fn Chat() to Element:
    # use_state returns (T, fn(T)->Unit)
    let (messages, set_messages) = use_state([])
    let (input, set_input) = use_state("")

    # Lambda syntax fn(arg) body
    let send = fn(_e) set_messages(messages.append({role: "user", text: input}))

    <div class="chat-container">
        <h1>"Vox Chatbot"</h1>
        <div class="messages">
            for msg in messages:
                <div class="message">
                    {msg.text}
                </div>
        </div>
        <div class="input-area">
            # Using input value requires valid property access
            # Currently builtins support generic field access
            <input class="chat-input" value={input}/>
            <button class="send-btn" on_click={send}>"Send"</button>
        </div>
    </div>

http post "/api/chat" to ChatResult:
    let body = request.json()
    # body.message access should infer as generic var
    let prompt = str(body.message)

    # spawn(Actor) returns a handle? (Not fully typed yet, generic var)
    # let response = spawn(Claude).send(prompt)

    Success("Hello " + prompt)

actor Claude:
    on send(msg: str) to ChatResult:
        Success("Hello from Vox! You said: " + msg)
"#;

    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Chatbot integration test failed with errors: {:?}",
        errs
    );
}
