use anyhow::{Context, Result};
use std::path::PathBuf;

const CHATBOT_TEMPLATE: &str = r#"# Vox Chatbot — OpenRouter-powered chat app
#
# Edit OPENROUTER_API_KEY in .env to enable live LLM responses.
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Conversation:
    user_id: str
    started_at: str

@table type MessageTrace:
    conversation_id: str
    role: str
    content: str
    request_id: str

@query fn recent_messages(conversation_id: str) to list[MessageTrace]:
    ret []

@mutation fn log_message(conversation_id: str, role: str, content: str, request_id: str) to Result[bool]:
    ret Ok(true)

activity call_provider(prompt: str) to Result[str]:
    Ok("stub-response: " + prompt)

workflow chat_pipeline(prompt: str) to Result[str]:
    let response = call_provider(prompt) with { retries: 3, timeout: "30s", activity_id: "provider-call" }
    response

@server fn chat(prompt: str, request_id: str) to Result[str]:
    let response = chat_pipeline(prompt) with { retries: 2, timeout: "45s" }
    let _ = log_message("conv-default", "user", prompt, request_id)
    let _ = log_message("conv-default", "assistant", "ok", request_id)
    ret response

@component fn Chat() to Element:
    let (messages, set_messages) = use_state([])
    let (input, set_input) = use_state("")
    <div class="chat-root">
        <h1>"Chat"</h1>
        <div class="messages">
            for msg in messages:
                <div class="message">{msg.content}</div>
        </div>
        <input value={input} on_change={set_input} placeholder="Type a message..." />
        <button on_click={fn():
            let _ = chat(input, "req-1")
        }>"Send"</button>
    </div>

routes:
    "/" to Chat
"#;

const DASHBOARD_TEMPLATE: &str = r#"# Vox Dashboard — data table with route params
#
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Item:
    name: str
    value: int
    created_at: str

@query fn list_items() to list[Item]:
    ret []

@mutation fn add_item(name: str, value: int) to Result[str]:
    ret Ok("Added: " + name)

@health fn check_db() to bool:
    ret true

@metric fn items_created(name: str) to str:
    ret "ok"

@component fn Dashboard() to Element:
    let (items, set_items) = use_state([])
    <div class="dashboard">
        <h1>"Dashboard"</h1>
        <table>
            for item in items:
                <tr>
                    <td>{item.name}</td>
                    <td>{item.value}</td>
                </tr>
        </table>
    </div>

@component fn ItemDetail() to Element:
    let id = use_param("id")
    <div>
        <h2>"Item: " + id</h2>
    </div>

routes:
    "/" to Dashboard
    "/items/:id" to ItemDetail
"#;

const API_TEMPLATE: &str = r#"# Vox API — server functions with health + metrics
#
# Run with: vox build src/main.vox -o dist && vox run src/main.vox

@table type Task:
    title: str
    done: bool
    created_at: str

@health fn health_check() to bool:
    ret true

@metric fn tasks_created() to str:
    ret "ok"

@server fn create_task(title: str) to Result[str]:
    ret Ok("Created: " + title)

@server fn list_tasks() to Result[list[Task]]:
    ret Ok([])

@server fn complete_task(id: str) to Result[bool]:
    ret Ok(true)
"#;

/// `vox init` — scaffold a new Vox project with a `Vox.toml` manifest.
pub async fn run(name: Option<&str>, kind: Option<&str>, template: Option<&str>) -> Result<()> {
    let project_name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .as_deref()
            .unwrap_or("my-project")
            .to_string()
            .leak()
    });
    let package_kind = kind.unwrap_or("application");

    if package_kind == "skill" {
        let skill_file_name = format!("{}.skill.md", project_name);
        let target_path = PathBuf::from(&skill_file_name);
        if target_path.exists() {
            anyhow::bail!("{} already exists in this directory", skill_file_name);
        }

        let content = format!(
            r#"---
id = "vox.{name}"
name = "{name}"
version = "0.1.0"
author = "your-name"
description = "A new Vox skill"
category = "custom:misc"
tools = ["vox_my_new_tool"]
tags = ["custom", "vox"]
permissions = []
---

# {name} Skill

Provide instructions for the Vox LLM on how to use this skill here.

## Tools

- `vox_my_new_tool` — What this tool does.

## Instructions

1. Use `vox_my_new_tool` when you need to...
2. Next steps...
"#,
            name = project_name
        );

        std::fs::write(&target_path, content)
            .with_context(|| format!("Failed to write {}", skill_file_name))?;
        println!("✓ Initialized Vox skill `{}`", project_name);
        println!("  Created {}", skill_file_name);
        println!();
        println!("  Next steps:");
        println!("    1. Edit {}", skill_file_name);
        println!("    2. Install with: vox skill install {}", skill_file_name);
        return Ok(());
    }

    let manifest_path = PathBuf::from("Vox.toml");
    if manifest_path.exists() {
        anyhow::bail!("Vox.toml already exists in this directory");
    }

    let manifest = vox_pm::VoxManifest::scaffold(project_name, package_kind);
    let toml_content = manifest
        .to_toml_string()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    std::fs::write(&manifest_path, &toml_content).with_context(|| "Failed to write Vox.toml")?;

    // Create default directory structure
    let src_dir = PathBuf::from("src");
    if !src_dir.exists() {
        std::fs::create_dir_all(&src_dir).with_context(|| "Failed to create src/ directory")?;
    }

    // Create a minimal main.vox (or use template)
    let main_file = src_dir.join("main.vox");
    if !main_file.exists() {
        let main_content: &str = if let Some(tmpl) = template {
            match tmpl {
                "chatbot" => CHATBOT_TEMPLATE,
                "dashboard" => DASHBOARD_TEMPLATE,
                "api" => API_TEMPLATE,
                other => {
                    eprintln!("⚠ Unknown template '{other}'. Choose: chatbot, dashboard, api");
                    eprintln!("  Falling back to default application template.");
                    "# My Vox App\n\n@component fn App() to Element:\n    <div><h1>\"My Vox App\"</h1></div>\n\nroutes:\n    \"/\" to App\n"
                }
            }
        } else {
            match package_kind {
                "agent" => "# A Vox AI agent\n\n@agent_def fn MyAgent(query: str) to str:\n    \"Response to: \" + query\n",
                "workflow" => "# A Vox workflow\n\nactivity process_data(input: str) to Result[str]:\n    Ok(\"Processed: \" + input)\n\nworkflow my_workflow(input: str) to Result[str]:\n    let result = process_data(input) with { retries: 3, timeout: \"10s\" }\n    result\n",
                "chatbot" => CHATBOT_TEMPLATE,
                _ => "# My Vox App — a full-stack starter\n#\n# Run with: vox build src/main.vox -o dist && vox run src/main.vox\n\n@table type Note:\n    title: str\n    content: str\n    created_at: str\n\n@server fn add_note(title: str, content: str) to Result[str]:\n    ret Ok(\"Added: \" + title)\n\n@server fn list_notes() to Result[str]:\n    ret Ok(\"[]\")\n\n@component fn App() to Element:\n    let (notes, set_notes) = use_state([])\n    <div class=\"app\">\n        <h1>\"My Vox App\"</h1>\n        <p>\"Edit src/main.vox to get started\"</p>\n    </div>\n\nroutes:\n    \"/\" to App\n",
            }
        };
        std::fs::write(&main_file, main_content).with_context(|| "Failed to write src/main.vox")?;
    }

    // Create .vox_modules directory
    let modules_dir = PathBuf::from(".vox_modules");
    if !modules_dir.exists() {
        std::fs::create_dir_all(&modules_dir).with_context(|| "Failed to create .vox_modules/")?;
    }

    let template_note = if template.is_some() {
        format!(" (template: {})", template.unwrap())
    } else {
        String::new()
    };

    println!("✓ Initialized Vox {} `{}`{}", package_kind, project_name, template_note);
    println!("  Created Vox.toml");
    println!("  Created src/main.vox");
    println!("  Created .vox_modules/");
    println!();
    match (package_kind, template) {
        (_, Some("chatbot")) | ("chatbot", _) => {
            println!("  Next steps:");
            println!("    1. Add OPENROUTER_API_KEY to .env");
            println!("    2. vox build src/main.vox -o dist");
            println!("    3. POST to http://localhost:3001/api/chat");
        }
        (_, Some("dashboard")) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. Open http://localhost:3000");
        }
        (_, Some("api")) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. curl http://localhost:3001/health");
        }
        ("application", _) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. vox run src/main.vox");
            println!("    3. Open http://localhost:3000");
        }
        ("workflow", _) => {
            println!("  Next steps:");
            println!("    1. vox build src/main.vox -o dist");
            println!("    2. Edit activities and workflow steps");
        }
        _ => {
            println!("  Get started with: vox build src/main.vox");
        }
    }

    Ok(())
}
