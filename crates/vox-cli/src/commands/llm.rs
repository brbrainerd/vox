use clap::Subcommand;
use owo_colors::OwoColorize;

#[derive(Subcommand)]
pub enum LlmCmd {
    /// Print relevant vox-language-surface.v1.json context + golden examples to stdout for use with any LLM.
    Prompt {
        /// The task you want help with (e.g., 'web-route', 'server-fn').
        task: String,
    },
}

pub async fn run(cmd: LlmCmd) -> anyhow::Result<()> {
    match cmd {
        LlmCmd::Prompt { task } => {
            println!(
                "{}",
                format!("Generating LLM prompt context for task: {}", task).bright_cyan()
            );

            let mut found = false;
            let task_lower = task.to_lowercase();

            if task_lower == "web-route" || task_lower == "route" || task_lower == "@query" {
                println!("{}", "--- Route Decorator Syntax ---".bright_yellow());
                println!("query get_user(id: u64) -> User {{\n    // ...\n}}");
                println!();
                println!("{}", "--- Golden Example ---".bright_yellow());
                println!(
                    "@query\npub fn get_profile() -> Result<Profile, Error> {{\n    Ok(Profile {{ name: \"Test\".to_string() }})\n}}"
                );
                println!();
                println!("{}", "--- MCP Schema Excerpt ---".bright_yellow());
                println!("{{ \"type\": \"route\", \"decorator\": \"@query\" }}");
                found = true;
            } else if task_lower == "server-fn"
                || task_lower == "mutation"
                || task_lower == "@mutation"
            {
                println!("{}", "--- Mutation Decorator Syntax ---".bright_yellow());
                println!(
                    "mutation update_user(id: u64, name: String) -> Result<(), Error> {{\n    // ...\n}}"
                );
                println!();
                println!("{}", "--- Golden Example ---".bright_yellow());
                println!(
                    "@mutation\npub fn update_profile(name: String) -> Result<(), Error> {{\n    Ok(())\n}}"
                );
                println!();
                println!("{}", "--- MCP Schema Excerpt ---".bright_yellow());
                println!("{{ \"type\": \"mutation\", \"decorator\": \"@mutation\" }}");
                found = true;
            }

            if !found {
                println!(
                    "No specific golden found for task: '{}'. Please refer to `docs/agents/vox-language-surface.v1.json`.",
                    task
                );
            }
        }
    }
    Ok(())
}
