pub mod autofix;
pub mod cli_actions;
mod commands;
pub mod fs_utils;
pub mod isolation;
pub mod pipeline;
pub mod slo_gates;
pub mod templates;
pub mod training;
pub mod v0;

use clap::Parser;

/// Build version string: `0.x.y+build.N (githash)`
const VOX_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+build.",
    env!("VOX_BUILD_NUMBER"),
    " (",
    env!("VOX_GIT_HASH"),
    ")",
);

#[derive(Parser)]
#[command(name = "vox", about = "The Vox AI-native language compiler", version = VOX_VERSION)]
enum Cli {
    /// Build a Vox source file, producing TypeScript output
    Build {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Output directory for generated files
        #[arg(short, long, default_value = "dist")]
        out_dir: std::path::PathBuf,
    },
    /// Type-check a Vox source file without producing output
    Check {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Run tests for the Vox program
    Test {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Profile script/build/run latency phases (lex, parse, typecheck, codegen, rust build, startup)
    Profile {
        /// Path to a script .vox file (must have fn main())
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Output timings as JSON
        #[arg(long, default_value = "false")]
        json: bool,
        /// Skip cache, force full recompile
        #[arg(long, default_value = "false")]
        no_cache: bool,
    },
    /// Run a Vox source file — auto-detects script vs web-app mode
    Run {
        /// Path to the .vox source file (omit when using --eval)
        #[arg(required_unless_present = "eval")]
        file: Option<std::path::PathBuf>,
        /// Arguments to pass to the application
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// Force script mode (skip web-app detection)
        #[arg(long, default_value = "false")]
        script: bool,
        /// Restrict syscalls: no network, no writes outside CWD
        #[arg(long, default_value = "false")]
        sandbox: bool,
        /// Grant access to the Vox MCP/agent APIs
        #[arg(long, default_value = "false")]
        allow_mcp: bool,
        /// Evaluate a Vox expression inline (no file required)
        #[arg(long)]
        eval: Option<String>,
        /// Skip the compilation cache, always recompile
        #[arg(long, default_value = "false")]
        no_cache: bool,
    },
    /// Bundle a Vox source file into a complete web application
    Bundle {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
        /// Output directory for generated files
        #[arg(short, long, default_value = "dist")]
        out_dir: std::path::PathBuf,
        /// Compile for a specific target triple (e.g. x86_64-unknown-linux-gnu)
        #[arg(long)]
        target: Option<String>,
        /// Build in release mode (default: true)
        #[arg(long, default_value = "true")]
        release: bool,
    },
    /// Format a Vox source file in place
    Fmt {
        /// Path to the .vox source file
        #[arg(required = true)]
        file: std::path::PathBuf,
    },
    /// Install a component or package via vox-pm
    Install {
        /// Name of package to retrieve
        #[arg(required = true)]
        package_name: String,
    },
    /// Start the Vox Language Server
    Lsp,
    /// Fine-tune a model on Vox corpus data
    Train {
        /// Use the native Burn/wgpu training engine (default; recommended for ≤7B models)
        #[arg(long, default_value = "true")]
        native: bool,
        /// HuggingFace model repo to fine-tune (e.g. Qwen/Qwen2.5-Coder-1.5B-Instruct).
        /// When set, weights are downloaded natively via hf-hub before training.
        #[arg(long)]
        model: Option<String>,
        /// GPU backend for native training: vulkan, dx12, metal, or cpu
        #[arg(long)]
        device: Option<String>,
        /// Directory containing train.jsonl (produced by `vox corpus pairs`)
        #[arg(long)]
        data_dir: Option<std::path::PathBuf>,
        /// Where to save the trained adapter / checkpoint
        #[arg(long)]
        output_dir: Option<std::path::PathBuf>,
        /// Training provider: local (Python QLoRA, 7B+ only), together, replicate
        #[arg(long)]
        provider: Option<String>,
    },
    /// Manage global Vox configuration
    #[command(subcommand)]
    Config(cli_actions::ConfigAction),
    /// Manage Vox agents
    #[command(subcommand)]
    Agent(cli_actions::AgentAction),
    /// Orchestrator control (agents, queues, tasks)
    #[command(subcommand)]
    Orchestrator(cli_actions::OrchestratorAction),
    /// Inspect and run Vox workflows
    #[command(subcommand)]
    Workflow(cli_actions::WorkflowAction),
    /// Native training data pipeline
    #[command(subcommand)]
    Corpus(commands::corpus::CorpusAction),
    /// Run TOESTUB anti-pattern detection (stubs, magic values, missing docs, etc.)
    StubCheck {
        /// Path to scan (default: current directory)
        #[arg(default_value = ".", value_name = "PATH")]
        path: std::path::PathBuf,
        /// Output format: terminal, json, markdown, grouped-json, sarif
        #[arg(short, long)]
        format: Option<String>,
        /// Minimum severity: info, warning, error, critical
        #[arg(short = 's', long)]
        severity: Option<String>,
        /// Generate fix suggestions and task queue
        #[arg(long, default_value = "true")]
        suggest_fixes: bool,
        /// Only run rules matching these comma-separated prefixes (e.g. stub,doc)
        #[arg(long)]
        rules: Option<String>,
        /// Exclude paths (glob patterns)
        #[arg(long)]
        excludes: Vec<String>,
        /// Languages to scan (comma-separated: rust,ts,python,gdscript,vox). Omit for all.
        #[arg(long)]
        langs: Option<String>,
        /// Baseline name or path. If a name (no path chars), load from VoxDB. If a path (e.g. .json), load from file. Exit 1 on new/regressed findings.
        #[arg(long)]
        baseline: Option<String>,
        /// Save current findings as a named baseline in VoxDB (e.g. --save-baseline main)
        #[arg(long)]
        save_baseline: Option<String>,
        /// Show last saved task queue from VoxDB
        #[arg(long)]
        task_list: bool,
        /// Import suppressions from toestub.toml into VoxDB (then exit)
        #[arg(long)]
        import_suppressions: bool,
        /// Ingest findings from a JSON file (e.g. toestub output) and save task queue to VoxDB (then exit)
        #[arg(long)]
        ingest_findings: Option<std::path::PathBuf>,
        /// Run staged fix pipeline (A: frontmatter, B: placeholders, C: unwired, D: sprawl) and report
        #[arg(long)]
        fix_pipeline: bool,
        /// With --fix-pipeline: apply Pass A (add missing frontmatter to Markdown files)
        #[arg(long)]
        fix_pipeline_apply: bool,
        /// CI gate: error-only (fail on critical/error), warnings (fail if warning budget exceeded), ratchet (save current counts as new budget)
        #[arg(long, value_name = "MODE")]
        gate: Option<String>,
        /// Path to warning budget JSON (by_rule counts). Used with --gate warnings or written with --gate ratchet.
        #[arg(long, value_name = "PATH")]
        gate_budget_path: Option<std::path::PathBuf>,
    },
    /// Run Fabrica (orchestrator dashboard: agents, tasks, Populi)
    #[cfg(feature = "dashboard")]
    #[command(alias = "dashboard")]
    Dash,
    /// AI provider usage, remaining budget, and cost summary
    #[cfg(feature = "dashboard")]
    Status {
        /// Output as JSON
        #[arg(long, default_value = "false")]
        json: bool,
        /// Force refresh OpenRouter catalog before showing status
        #[arg(long, default_value = "false")]
        refresh_catalog: bool,
    },
    /// Deploy to container, bare-metal, compose, or Kubernetes
    Deploy {
        /// Environment name (e.g. production, staging)
        #[arg(required = true)]
        env: String,
        /// Path to Vox.toml (default: current dir)
        #[arg(long)]
        config: Option<std::path::PathBuf>,
        /// Override target: container, bare-metal, compose, k8s
        #[arg(long)]
        target: Option<String>,
        /// Show plan without executing
        #[arg(long, default_value = "false")]
        dry_run: bool,
        /// Build inside hermetic container
        #[arg(long, default_value = "false")]
        hermetic: bool,
    },
    /// Run the REST execution API (scripts/run, scripts/build, jobs, artifacts)
    #[cfg(feature = "execution-api")]
    Serve {
        /// Bind address (default: 127.0.0.1:3848)
        #[arg(long, default_value = "127.0.0.1:3848")]
        addr: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    match cli {
        Cli::Build { file, out_dir } => {
            commands::build::run(&file, &out_dir).await?;
        }
        Cli::Check { file } => {
            commands::check::run(&file).await?;
        }
        Cli::Run { file, args, script, sandbox, allow_mcp, eval, no_cache } => {
            // --eval mode: no file needed
            if let Some(expr) = eval {
                commands::script::eval_inline(&expr, sandbox).await?;
            } else {
                let file = file.expect("file is required when --eval is not used");
                // Auto-detect script mode or use --script flag
                let is_script = script || commands::run::is_script_file(&file);
                if is_script {
                    let opts = commands::script::ScriptOpts {
                        sandbox,
                        allow_mcp,
                        no_cache,
                    };
                    commands::script::run(&file, &args, &opts).await?;
                } else {
                    commands::run::run(&file, &args).await?;
                }
            }
        }
        Cli::Test { file } => {
            commands::test::run(&file).await?;
        }
        Cli::Profile { file, json, no_cache } => {
            commands::profile::run(&file, json, no_cache).await?;
        }
        Cli::Bundle {
            file,
            out_dir,
            target,
            release,
        } => {
            commands::bundle::run(&file, &out_dir, target.as_deref(), release).await?;
        }
        Cli::Fmt { file } => {
            commands::fmt::run(&file, false)?;
        }
        Cli::Install { package_name } => {
            commands::install::run(Some(&package_name), false).await?;
        }
        Cli::Lsp => {
            commands::lsp::run()?;
        }
        Cli::Train {
            native,
            model,
            device,
            data_dir,
            output_dir,
            provider,
        } => {
            // Set VOX_BACKEND from --device flag so native.rs::make_wgpu_device() picks it up
            if let Some(ref d) = device {
                // SAFETY: called single-threaded before tokio::main spawns any workers
                unsafe { std::env::set_var("VOX_BACKEND", d); }
            }
            // Set VOX_BASE_MODEL for the native training path
            if let Some(ref m) = model {
                // SAFETY: called single-threaded before tokio::main spawns any workers
                unsafe { std::env::set_var("VOX_BASE_MODEL", m); }
            }
            commands::train::run(data_dir, output_dir, provider, native).await?;
        }
        Cli::Config(action) => {
            match action {
                cli_actions::ConfigAction::Get { key } => {
                    let config = vox_config::VoxConfig::load();
                    if let Some(val) = config.get_key(&key) {
                        println!("{val}");
                    } else {
                        eprintln!("Key '{key}' not found");
                        std::process::exit(1);
                    }
                }
                cli_actions::ConfigAction::Set { key, value } => {
                    let mut config = vox_config::VoxConfig::load();
                    if !config.set_key(&key, &value) {
                        eprintln!("Unknown configuration key '{key}'");
                        std::process::exit(1);
                    }
                    if let Err(e) = config.save() {
                        eprintln!("Failed to save config: {e}");
                        std::process::exit(1);
                    }
                    println!("Set {key} = {value}");
                }
                cli_actions::ConfigAction::List => {
                    let config = vox_config::VoxConfig::load();
                    let map = config.to_map();
                    for (k, v) in map {
                        println!("{k} = {v}");
                    }
                }
            }
        }
        Cli::Agent(action) => {
            match action {
                cli_actions::AgentAction::Status { id } => {
                    // Status delegates to orchestrator status when no id filter
                    if id.is_none() {
                        commands::orchestrator::status().await?;
                    } else {
                        println!("Agent status (filtered by id) — use `vox orchestrator status` for full view.");
                        if let Some(agent_id) = id {
                            commands::orchestrator::queue(agent_id).await?;
                        }
                    }
                }
                cli_actions::AgentAction::List => {
                    commands::agent::list().await?;
                }
                cli_actions::AgentAction::Create { name, description, system_prompt, tools, model_config, public } => {
                    commands::agent::create(
                        &name,
                        description.as_deref(),
                        system_prompt.as_deref(),
                        tools.as_deref(),
                        model_config.as_deref(),
                        public,
                    ).await?;
                }
                cli_actions::AgentAction::Info { id } => {
                    commands::agent::info(&id).await?;
                }
                cli_actions::AgentAction::Generate => {
                    commands::agent::generate().await?;
                }
            }
        }
        Cli::Orchestrator(action) => {
            match action {
                cli_actions::OrchestratorAction::Status => {
                    commands::orchestrator::status().await?;
                }
                cli_actions::OrchestratorAction::Hud => {
                    commands::hud::run().await?;
                }
                cli_actions::OrchestratorAction::Submit { description, files, priority } => {
                    commands::orchestrator::submit(&description, &files, priority.as_deref()).await?;
                }
                cli_actions::OrchestratorAction::Queue { agent_id } => {
                    commands::orchestrator::queue(agent_id).await?;
                }
                cli_actions::OrchestratorAction::Rebalance => {
                    commands::orchestrator::rebalance().await?;
                }
                cli_actions::OrchestratorAction::Config => {
                    commands::orchestrator::config().await?;
                }
                cli_actions::OrchestratorAction::Pause { agent_id } => {
                    commands::orchestrator::pause(agent_id).await?;
                }
                cli_actions::OrchestratorAction::Resume { agent_id } => {
                    commands::orchestrator::resume(agent_id).await?;
                }
                cli_actions::OrchestratorAction::Save => {
                    commands::orchestrator::save().await?;
                }
                cli_actions::OrchestratorAction::Load => {
                    commands::orchestrator::load().await?;
                }
                cli_actions::OrchestratorAction::Undo { count } => {
                    commands::orchestrator::undo(count).await?;
                }
                cli_actions::OrchestratorAction::Redo { count } => {
                    commands::orchestrator::redo(count).await?;
                }
            }
        }
        Cli::Workflow(action) => {
            match action {
                cli_actions::WorkflowAction::List { file } => {
                    commands::workflow::list(&file).await?;
                }
                cli_actions::WorkflowAction::Inspect { file, name } => {
                    commands::workflow::inspect(&file, &name).await?;
                }
                cli_actions::WorkflowAction::Check { file } => {
                    commands::workflow::check(&file).await?;
                }
                cli_actions::WorkflowAction::Run { file, name } => {
                    commands::workflow::run(&file, &name).await?;
                }
            }
        }
        Cli::Corpus(action) => {
            commands::corpus::run(action).await?;
        }
        Cli::StubCheck {
            path,
            format,
            severity,
            suggest_fixes,
            rules,
            excludes,
            langs,
            baseline,
            save_baseline,
            task_list,
            import_suppressions,
            ingest_findings,
            fix_pipeline,
            fix_pipeline_apply,
            gate,
            gate_budget_path,
        } => {
            commands::stub_check::run(
                &path,
                format.as_deref(),
                severity.as_deref(),
                suggest_fixes,
                rules.as_deref(),
                &excludes,
                langs.as_deref(),
                baseline.as_deref(),
                save_baseline.as_deref(),
                task_list,
                import_suppressions,
                ingest_findings.as_deref(),
                fix_pipeline,
                fix_pipeline_apply,
                gate.as_deref(),
                gate_budget_path.as_deref(),
            )
            .await?;
        }
        #[cfg(feature = "dashboard")]
        Cli::Dash => {
            commands::dashboard::run().await?;
        }
        #[cfg(feature = "dashboard")]
        Cli::Status { json, refresh_catalog } => {
            commands::status::run(json, refresh_catalog).await?;
        }
        Cli::Deploy { env, config, target, dry_run, hermetic } => {
            commands::deploy::run(&env, config.as_deref(), target.as_deref(), dry_run, hermetic).await?;
        }
        #[cfg(feature = "execution-api")]
        Cli::Serve { addr } => {
            commands::execution::serve(&addr).await?;
        }
    }

    Ok(())
}
