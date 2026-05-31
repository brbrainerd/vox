//! Subcommand dispatch and fabrica / Latin lane helpers.

mod lanes;

use crate::codex_cmd::CodexCmd;
use crate::command_catalog;
// use crate::latin_cmd; // Unused after alias retirement
use crate::{Cli, GlobalOpts, VoxCliRoot};

#[cfg(feature = "ars")]
pub(crate) use lanes::run_openclaw_subcommand;
#[cfg(feature = "coderabbit")]
pub(crate) use lanes::run_review_subcommand;
#[cfg(feature = "script-execution")]
pub(crate) use lanes::run_script_subcommand;
#[cfg(feature = "stub-check")]
pub(crate) use lanes::run_stub_check_command;
pub(crate) use lanes::{
    cli_top_level_into_fabrica_or_self, run_ars_cmd, run_diag_cmd, run_doctor_command,
    run_fabrica_cmd,
};

pub(crate) async fn dispatch_cli(cli: Cli, global: &GlobalOpts) -> anyhow::Result<()> {
    {
        let _ = global;
    }
    let cli = match cli_top_level_into_fabrica_or_self(cli) {
        Ok(cmd) => return run_fabrica_cmd(cmd).await,
        Err(cli) => cli,
    };
    match cli {
        // Compiler cannot narrow `Cli` after [`cli_top_level_into_fabrica_or_self`]; these are unreachable.
        Cli::Build { .. }
        | Cli::Check { .. }
        | Cli::Test { .. }
        | Cli::Run { .. }
        | Cli::Dev { .. }
        | Cli::BundleApp { .. }
        | Cli::Compile { .. }
        | Cli::Fmt { .. } => {
            std::unreachable!("top-level fabrica shims are routed before this match")
        }
        Cli::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = VoxCliRoot::command();
            clap_complete::generate(shell, &mut cmd, "vox", &mut std::io::stdout());
        }
        Cli::Emit { cmd } => {
            crate::commands::emit::run(cmd).await?;
        }
        Cli::Commands {
            format,
            recommended,
            include_nested,
            search,
        } => {
            let catalog = command_catalog::build_catalog();
            let generated_from = catalog.generated_from.clone();
            // `--search` implies --include-nested so results span the full tree.
            let effective_include_nested = include_nested || search.is_some();
            let pre_search = command_catalog::select_entries(
                catalog.entries,
                recommended,
                effective_include_nested,
            );
            // Compute scored results once so both Text and Json formats see the same data.
            let scored: Option<Vec<command_catalog::SearchResult>> = search
                .as_ref()
                .map(|p| command_catalog::search_entries_scored(pre_search.clone(), p));
            let selected: Vec<command_catalog::CommandCatalogEntry> = match &scored {
                Some(s) => s.iter().map(|sr| sr.entry.clone()).collect(),
                None => pre_search,
            };
            if let Some(ref pattern) = search {
                if selected.is_empty() {
                    eprintln!("vox commands: no matches for {:?}", pattern);
                }
            }
            match format {
                command_catalog::CatalogFormat::Text => {
                    let text = if let Some(ref pattern) = search {
                        command_catalog::render_search_results(&selected, pattern)
                    } else {
                        command_catalog::render_text(&selected)
                    };
                    println!("{text}");
                }
                command_catalog::CatalogFormat::Json => {
                    if let (Some(pattern), Some(results)) = (&search, scored) {
                        let out = command_catalog::SearchOutput {
                            generated_from,
                            pattern: pattern.clone(),
                            match_count: results.len(),
                            results,
                        };
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    } else {
                        let out = command_catalog::CommandCatalog {
                            generated_from,
                            entries: selected,
                        };
                        println!("{}", serde_json::to_string_pretty(&out)?);
                    }
                }
            }
        }
        Cli::Fabrica { cmd } => {
            run_fabrica_cmd(cmd).await?;
        }
        Cli::Diag { cmd } => {
            run_diag_cmd(cmd).await?;
        }
        Cli::Ext { cmd } => {
            crate::commands::ext::run(cmd).await?;
        }
        Cli::Ars { cmd } => {
            run_ars_cmd(cmd).await?;
        }
        #[cfg(feature = "extras-ludus")]
        Cli::Ludus { cmd } => {
            crate::commands::extras::ludus_cli::run(cmd).await?;
        }
        Cli::Secrets { cmd } => {
            crate::commands::secrets::run(cmd).await?;
        }
        Cli::Auth { cmd } => {
            crate::commands::auth::run(cmd).await?;
        }
        Cli::Config { cmd } => {
            crate::commands::config::run(cmd).await?;
        }
        #[cfg(feature = "coderabbit")]
        Cli::Recensio { cmd } => {
            run_review_subcommand(cmd).await?;
        }
        Cli::Audit { args } => {
            // F1: route the nested `vox audit effort` subcommand on the async
            // path; the flag-based check-targets / CR-L dispatch stays sync.
            match args.command.clone() {
                Some(crate::commands::audit::AuditSubcommand::Effort(eff)) => {
                    crate::commands::audit_effort::run(eff).await?;
                }
                Some(crate::commands::audit::AuditSubcommand::EffortRoute(route)) => {
                    crate::commands::audit_route::run(route).await?;
                }
                None => {
                    crate::commands::audit::run(&args)?;
                }
            }
        }
        Cli::Ci { cmd } => {
            crate::commands::ci::run(cmd).await?;
        }
        #[cfg(feature = "script-execution")]
        Cli::Script { args } => {
            run_script_subcommand(&args, "top-level").await?;
        }
        #[cfg(not(feature = "script-execution"))]
        Cli::ScriptStub { .. } => {
            anyhow::bail!(
                "{}\n\nThis binary was built without the `script-execution` cargo feature. \
                 Resolve by either (a) installing the runtime plugin or (b) rebuilding with the feature:\n\n{}",
                "vox script requires the 'script-execution' capability, which is not available in this build.",
                vox_plugin_host::format_install_hint(
                    "script-execution",
                    Some("cargo build -p vox-cli --release --features script-execution")
                )
            );
        }
        #[cfg(feature = "live")]
        Cli::Live => {
            crate::commands::live::run().await?;
        }
        Cli::Add { args } => {
            crate::commands::add::run(&args.name, args.version.as_deref(), args.path.as_deref())
                .await?;
        }
        Cli::Remove { args } => {
            crate::commands::remove::run(&args.name).await?;
        }
        Cli::Update => {
            crate::commands::update::run().await?;
        }
        Cli::Lock { args } => {
            crate::commands::lock::run(args.locked).await?;
        }
        Cli::Sync { args } => {
            crate::commands::sync::run(args.registry.as_deref(), args.frozen).await?;
        }
        Cli::Login { args } => {
            crate::commands::login_shared::run_login(args.into()).await?;
        }
        Cli::Logout => {
            crate::commands::login_shared::run_logout().await?;
        }
        Cli::Share { args } => {
            crate::commands::share::run(args).await?;
        }
        Cli::Train { .. } => {
            eprintln!("vox train is deprecated. Use `vox mens train` instead.");
            std::process::exit(1);
        }
        Cli::Snippet { cmd } => {
            crate::commands::extras::snippet_cli::run(cmd).await?;
        }
        #[cfg(feature = "ars")]
        Cli::Skill { cmd } => {
            crate::commands::extras::skill_cmd::run(cmd).await?;
        }
        Cli::Deploy { args } => {
            crate::commands::deploy::run(args).await?;
        }
        Cli::Pm { cmd } => {
            crate::commands::pm::run(cmd).await?;
        }
        Cli::Doctor { args } => {
            run_doctor_command(&args).await?;
        }
        #[cfg(any(feature = "codex", feature = "stub-check"))]
        Cli::Architect { cmd } => {
            crate::commands::diagnostics::tools::architect::run(cmd).await?;
        }
        #[cfg(feature = "stub-check")]
        Cli::StubCheck { args } => {
            run_stub_check_command(&args).await?;
        }
        Cli::Upgrade { args } => {
            crate::commands::upgrade::run(&args, global.json).await?;
        }
        Cli::Init {
            name,
            kind,
            template,
        } => {
            crate::commands::init::run(name.as_deref(), kind.as_deref(), template.as_deref())
                .await?;
        }
        Cli::New { cmd } => {
            crate::commands::new::run(cmd).await?;
        }
        Cli::Play { args } => {
            crate::commands::play::run(args).await?;
        }
        Cli::Repair { args } => {
            crate::commands::repair::run(args).await?;
        }
        Cli::Lsp => {
            crate::commands::lsp::run()?;
        }
        Cli::Migrate { cmd } => {
            crate::commands::migrate::run(cmd)?;
        }
        Cli::Mcp => {
            crate::commands::mcp::run().await?;
        }
        Cli::Shell { cmd } => {
            crate::commands::runtime::shell::run(cmd).await?;
        }
        Cli::Db { cmd } => {
            crate::commands::db_cli::run(cmd).await?;
        }
        Cli::Memory { cmd } => {
            crate::commands::memory_cli::run(cmd).await?;
        }
        Cli::Scientia { cmd } => {
            crate::commands::scientia::run(cmd).await?;
        }
        Cli::Model { cmd } => {
            crate::commands::model::run(cmd).await?;
        }
        Cli::Wasm { cmd } => {
            crate::commands::wasm::run(cmd)?;
        }
        #[cfg(feature = "dei")]
        Cli::Dei { cmd } => {
            crate::commands::dei::run(cmd).await?;
        }
        Cli::Codex { cmd } => match cmd {
            CodexCmd::Verify => {
                crate::commands::codex::verify().await?;
            }
            CodexCmd::ExportLegacy { output } => {
                crate::commands::codex::export_legacy(&output).await?;
            }
            CodexCmd::ImportLegacy { input } => {
                crate::commands::codex::import_legacy(&input).await?;
            }
            CodexCmd::ImportOrchestratorMemory {
                dir,
                agent_id,
                session_id,
            } => {
                crate::commands::codex::import_orchestrator_memory(dir, agent_id, session_id)
                    .await?;
            }
            CodexCmd::ImportSkillBundle { file } => {
                crate::commands::codex::import_skill_bundle(file).await?;
            }
            CodexCmd::Cutover {
                target_db,
                source_db,
                artifact_dir,
                force,
            } => {
                crate::commands::codex::cutover(artifact_dir, target_db, source_db, force).await?;
            }
            CodexCmd::SocratesMetrics {
                repository_id,
                limit,
            } => {
                crate::commands::codex::socrates_metrics(repository_id, limit).await?;
            }
            CodexCmd::SocratesEvalSnapshot {
                eval_id,
                repository_id,
                limit,
            } => {
                crate::commands::codex::socrates_eval_snapshot(eval_id, repository_id, limit)
                    .await?;
            }
        },
        #[cfg(feature = "dei")]
        Cli::Attention { cmd } => {
            crate::commands::attention::handle_attention_command(
                cmd,
                &std::env::current_dir().map_err(|e| anyhow::anyhow!("{}", e))?,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Cli::Repo { cmd } => {
            let cmd = cmd.unwrap_or(crate::commands::repo::RepoCmd::Status { json: false });
            crate::commands::repo::run(cmd).await?;
        }
        #[cfg(feature = "dei")]
        Cli::Safety { cmd } => {
            crate::commands::safety::handle_safety_command(
                cmd,
                &std::env::current_dir().map_err(|e| anyhow::anyhow!("{}", e))?,
            )
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Cli::Catalog { cmd } => {
            crate::commands::catalog::run(cmd).await?;
        }
        #[cfg(feature = "ars")]
        Cli::Openclaw { action } => {
            run_openclaw_subcommand(action).await?;
        }
        Cli::Stop { reason } => {
            #[cfg(feature = "dei")]
            crate::commands::dei::stop(reason).await?;
            #[cfg(not(feature = "dei"))]
            {
                let _ = reason;
                eprintln!("Feature 'dei' is not enabled.");
            }
        }

        Cli::Plan { cmd } => {
            crate::commands::plan::dispatch(cmd).await?;
        }
        Cli::Llm { cmd } => {
            crate::commands::llm::run(cmd).await?;
        }
        Cli::Generate { args } => {
            crate::commands::generate::run(
                &args.prompt,
                args.output,
                args.no_validate,
                args.server_url.as_deref(),
                args.max_retries,
                args.legacy_direct,
            )
            .await?;
        }
        #[cfg(feature = "dei")]
        Cli::Visus { cmd } => {
            crate::commands::visus::dispatch(cmd)
                .await
                .map_err(|e| anyhow::anyhow!("{:?}", e))?;
        }
        #[cfg(feature = "gui")]
        Cli::Gui { args } => {
            crate::commands::gui::run(args).await?;
        }
        Cli::DriftCheck { args } => {
            crate::commands::drift_check::run(args).await?;
        }
        Cli::Research { cmd } => crate::commands::research::run(cmd).await?,
        #[cfg(feature = "coderabbit")]
        Cli::Review { cmd } => {
            run_review_subcommand(cmd).await?;
        }
        Cli::Plugin { cmd } => {
            crate::commands::plugin::run(cmd).await?;
        }
        Cli::Bundle { cmd } => {
            crate::commands::plugin_bundle::run(cmd).await?;
        }
        Cli::Telemetry { cmd } => {
            crate::commands::telemetry::run(cmd).await?;
        }
        Cli::Snapshot { cmd } => {
            crate::commands::snapshot::run(&cmd)?;
        }
        Cli::Rollback { id } => {
            crate::commands::rollback::run(id).await?;
        }
        Cli::Workflow { cmd } => {
            crate::commands::workflow::run(cmd).await?;
        }
        Cli::Dispatch { cmd } => {
            crate::commands::dispatch::run(cmd).await?;
        }
        Cli::Grammar { args } => {
            crate::commands::grammar::handle(args);
        }
        Cli::Mens { .. } | Cli::Populi { .. } | Cli::Oratio { .. } | Cli::Schola { .. } => {
            std::unreachable!(
                "ML/AI commands are intercepted in main.rs and delegated to external binaries"
            )
        }
    }

    Ok(())
}
