use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use rand::seq::SliceRandom;
use rand::thread_rng;

/// A single prompt→response training pair.
/// Schema MUST match `vox-tensor`'s `JsonlDataLoader` — uses `prompt`/`response`/`rating`/`category`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrainingPair {
    /// The instruction / user prompt.
    pub prompt: String,
    /// The expected Vox code response.
    pub response: String,
    /// Construct category (function, actor, workflow, etc.)
    pub category: String,
    /// Quality rating 1-5. Synthetic pairs default to 4.
    pub rating: u8,
}

struct Example {
    instructions: &'static [&'static str],
    code: &'static str,
    category: &'static str,
}

const EXAMPLES: &[Example] = &[
    // Fundamental types / ADTs
    Example {
        instructions: &[
            "Define a Vox tagged union type called HttpStatus with Ok, NotFound, and ServerError variants",
            "Create an ADT in Vox for HTTP status codes with three variants",
            "Write a Vox type representing HTTP response statuses",
            "Show me how to model HTTP responses as a tagged union in Vox",
        ],
        code: "type HttpStatus =\n    | Ok(code: int)\n    | NotFound(path: str)\n    | ServerError(message: str)",
        category: "type",
    },
    // Basic functions
    Example {
        instructions: &[
            "Write a Vox function that computes the factorial of a number",
            "Create a factorial function in Vox",
            "Implement a recursive factorial in the Vox language",
            "Show me a recursive function in Vox",
        ],
        code: "fn factorial(n: int) to int:\n    if n <= 1:\n        ret 1\n    ret n * factorial(n - 1)",
        category: "function",
    },
    // Actors
    Example {
        instructions: &[
            "Write a Vox actor that counts messages and supports increment, decrement and get operations",
            "Create a counter actor in Vox with state management",
            "Implement a stateful counter using the Vox actor model",
            "Show me how to use actors in Vox for shared mutable state",
        ],
        code: "actor Counter:\n    state count: int = 0\n\n    on increment(amount: int) to int:\n        count = count + amount\n        count\n\n    on decrement(amount: int) to int:\n        count = count - amount\n        count\n\n    on get_count() to int:\n        count",
        category: "actor",
    },
    // Workflows
    Example {
        instructions: &[
            "Write a Vox workflow for processing a payment with validation, charge, and receipt steps",
            "Create a durable payment processing workflow in Vox with retry policies",
            "Show me how to write a multi-step durable workflow in Vox",
        ],
        code: "workflow process_payment(card_token: str, amount: int) to Result[str]:\n    let validated = validate_card(card_token) with { retries: 2 }\n    let tx_id = charge_amount(card_token, amount) with { retries: 3 }\n    ret Ok(\"Payment complete: \" + tx_id)",
        category: "workflow",
    },
    // Components
    Example {
        instructions: &[
            "Write a Vox UI component for a todo list",
            "Create a reactive todo list component in Vox using JSX syntax",
            "Show me how to write a stateful component in Vox",
        ],
        code: "@component fn TodoList() to Element:\n    let (items, set_items) = use_state([])\n    <div class=\"todo-container\">\n        <h1>\"Todo List\"</h1>\n        <ul>\n            for item in items:\n                <li>{item.text}</li>\n        </ul>\n    </div>",
        category: "component",
    },
    // Tables
    Example {
        instructions: &[
            "Define a Vox database table for storing user records",
            "Create a Users table in Vox with id, name, and email fields",
            "Show me how to declare a database table in Vox",
        ],
        code: "@table type User:\n    id: str\n    name: str\n    email: str\n    created_at: str",
        category: "table",
    },
    // Queries
    Example {
        instructions: &[
            "Write a Vox database query to list all users",
            "Create a read-only query in Vox that returns all User records",
            "Show me how to write a @query in Vox",
        ],
        code: "@query fn get_all_users() to [User]:\n    db.query(User).order_by(\"created_at\", \"desc\").all()",
        category: "query",
    },
    // MCP tools
    Example {
        instructions: &[
            "Write a Vox MCP tool that searches the codebase for a pattern",
            "Create an MCP-compatible search tool in Vox for AI assistants",
            "Show me how to expose a function as an MCP tool in Vox",
        ],
        code: "@mcp.tool(\"search_code\", \"Search the codebase for a pattern\")\nfn search_code(pattern: str) to [str]:\n    let results = fs.grep(pattern, \"**/*.vox\")\n    results.map(|r| r.path)",
        category: "mcp_tool",
    },
    // Activities
    Example {
        instructions: &[
            "Write a Vox activity that sends an email with retry support",
            "Create a retryable send_email activity in Vox",
            "Show me how to define a durable activity in Vox",
        ],
        code: "activity send_email(to: str, subject: str, body: str) to Result[Unit]:\n    let response = http.post(\"https://api.mail.io/send\", {\n        to: to,\n        subject: subject,\n        body: body,\n    })\n    if response.status == 200:\n        ret Ok(Unit)\n    ret Err(\"Send failed: \" + response.body)",
        category: "activity",
    },
    // Agent definitions
    Example {
        instructions: &[
            "Define a Vox AI agent that can search code and answer questions",
            "Create an agent definition in Vox with memory and tool access",
            "Show me how to declare an AI agent in Vox",
        ],
        code: "@agent_def fn CodeAssistant() to Agent:\n    system_prompt: \"You are an expert Vox programmer.\"\n    tools: [search_code, get_file, run_tests]\n    memory: episodic",
        category: "agent_def",
    },
];

pub fn generate_data<P: AsRef<Path>>(output_path: P, limit: Option<usize>) -> std::io::Result<()> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);
    let mut rng = thread_rng();

    let mut all_pairs = Vec::new();

    for example in EXAMPLES {
        for &inst in example.instructions {
            all_pairs.push(TrainingPair {
                prompt: inst.to_string(),
                response: example.code.to_string(),
                category: example.category.to_string(),
                rating: 5,
            });
        }
    }

    all_pairs.shuffle(&mut rng);

    let mut count = 0;
    for pair in all_pairs {
        if let Some(l) = limit {
            if count >= l { break; }
        }
        let json = serde_json::to_string(&pair)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writer.write_all(json.as_bytes())?;
        writer.write_all(b"\n")?;
        count += 1;
    }

    writer.flush()?;
    println!("✓ Generated {} training pairs (schema: prompt/response/rating/category)", count);
    Ok(())
}
