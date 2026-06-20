//! CLI commands for history and clip manager.

use anyhow::Context;
use dialoguer::{FuzzySelect, Select, theme::SimpleTheme};
use vox_db::history_store;

/// Execute the `vox clip` subcommand.
pub async fn run_clip(cmd: ClipSubCommand) -> anyhow::Result<()> {
    let db = crate::workspace_db::connect_cli_workspace_voxdb().await?;
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id.to_string();

    match cmd {
        ClipSubCommand::Add { text, source } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .context("system clock error")?
                .as_millis() as i64;
            let id =
                history_store::add_entry(&db, &repo_id, "clip", &text, "", now, &source).await?;
            println!("Clip added with ID: {}", id);
        }
        ClipSubCommand::List { limit } => {
            let entries = history_store::list_entries(&db, &repo_id, Some("clip"), limit).await?;
            print_entries_table(entries);
        }
    }
    Ok(())
}

/// Execute the `vox history` subcommand.
pub async fn run_history(cmd: HistorySubCommand) -> anyhow::Result<()> {
    let db = crate::workspace_db::connect_cli_workspace_voxdb().await?;
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repo_id = repo_ctx.repository_id.to_string();

    match cmd {
        HistorySubCommand::List { kind, limit } => {
            let entries =
                history_store::list_entries(&db, &repo_id, kind.as_deref(), limit).await?;
            print_entries_table(entries);
        }
        HistorySubCommand::Search { query, limit } => {
            let entries = history_store::search_entries(&db, &repo_id, &query, limit).await?;
            print_entries_table(entries);
        }
        HistorySubCommand::Delete { id } => {
            history_store::delete_entry(&db, id).await?;
            println!("Entry {} deleted.", id);
        }
        HistorySubCommand::Pin { id, pinned } => {
            history_store::pin_entry(&db, id, pinned).await?;
            println!("Entry {} pin state updated to {}.", id, pinned);
        }
        HistorySubCommand::Interactive { kind } => {
            let entries = history_store::list_entries(&db, &repo_id, kind.as_deref(), 100).await?;
            if entries.is_empty() {
                println!("No history entries found.");
                return Ok(());
            }

            let items: Vec<String> = entries
                .iter()
                .map(|e| {
                    let text = e.redacted_text.replace('\n', " ");
                    let snippet: String = text.chars().take(80).collect();
                    format!("[{}] (id: {}) {}", e.kind, e.id, snippet)
                })
                .collect();

            let selection = FuzzySelect::with_theme(&SimpleTheme)
                .with_prompt("Search history (type to filter, Enter to select, Esc to cancel)")
                .items(&items)
                .default(0)
                .interact_opt()?;

            if let Some(index) = selection {
                let entry = &entries[index];
                let mut action_choices = vec!["Copy to clipboard", "Print to stdout"];
                if entry.kind == "command" {
                    action_choices.push("Execute command");
                }
                action_choices.push("Cancel");

                let action_selection = Select::with_theme(&SimpleTheme)
                    .with_prompt(format!("Selected: {}", entry.redacted_text))
                    .items(&action_choices)
                    .default(0)
                    .interact()?;

                match action_choices[action_selection] {
                    "Copy to clipboard" => {
                        copy_to_clipboard(&entry.text)?;
                        println!("Copied to clipboard.");
                    }
                    "Print to stdout" => {
                        println!("{}", entry.text);
                    }
                    "Execute command" => {
                        if let Err(e) = crate::commands::runtime::shell::check_terminal::run_check(
                            &entry.text,
                            None,
                        ) {
                            eprintln!("Command execution blocked: {e}");
                        } else {
                            println!("Executing command: {}", entry.text);
                            execute_shell_command(&entry.text)?;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn print_entries_table(entries: Vec<history_store::HistoryEntry>) {
    use comfy_table::Table;
    let mut table = Table::new();
    table.set_header(vec!["ID", "Kind", "Pinned", "Text Snippet", "Created At"]);
    for e in entries {
        let pinned_str = if e.pinned { "Yes" } else { "No" };
        let text_preview = e.redacted_text.replace('\n', " ");
        let text_preview: String = text_preview.chars().take(60).collect();
        let time_str = chrono::DateTime::from_timestamp(
            e.created_at / 1000,
            ((e.created_at % 1000) * 1_000_000) as u32,
        )
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| e.created_at.to_string());
        table.add_row(vec![
            e.id.to_string(),
            e.kind,
            pinned_str.to_string(),
            text_preview,
            time_str,
        ]);
    }
    println!("{table}");
}

fn copy_to_clipboard(text: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("clip").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        } else if let Ok(mut child) = Command::new("xsel")
            .arg("--clipboard")
            .arg("--input")
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }
    Ok(())
}

fn execute_shell_command(cmd_str: &str) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("powershell")
            .arg("-Command")
            .arg(cmd_str)
            .status()?;
        if !status.success() {
            eprintln!("Command exited with status: {status}");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd_str)
            .status()?;
        if !status.success() {
            eprintln!("Command exited with status: {status}");
        }
    }
    Ok(())
}

#[derive(clap::Subcommand, Clone, Debug)]
pub enum ClipSubCommand {
    /// Add a new text clip to history.
    Add {
        /// Text to save.
        text: String,
        /// Source identifier (optional, defaults to "cli").
        #[arg(long, default_value = "cli")]
        source: String,
    },
    /// List recent clip entries.
    List {
        /// Maximum number of entries to display.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
}

#[derive(clap::Subcommand, Clone, Debug)]
pub enum HistorySubCommand {
    /// List recent history entries.
    List {
        /// Kind to filter by (clip, command, chat).
        #[arg(long)]
        kind: Option<String>,
        /// Maximum number of entries to display.
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Search history database.
    Search {
        /// Search query.
        query: String,
        /// Maximum number of entries.
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Delete a history entry by ID.
    Delete {
        /// Entry ID.
        id: i64,
    },
    /// Pin/unpin a history entry by ID.
    Pin {
        /// Entry ID.
        id: i64,
        /// Pin state (true to pin, false to unpin).
        #[arg(long, default_value_t = true)]
        pinned: bool,
    },
    /// Interactive fuzzy search with fuzzy matcher and menu select.
    Interactive {
        /// Kind to filter by (clip, command, chat).
        #[arg(long)]
        kind: Option<String>,
    },
}
