use owo_colors::OwoColorize;
use std::io::Write;
use std::path::Path;
use vox_db::VoxDb;

const EXPORT_DATASET_LIMIT: i64 = 10_000;
const DOGFOOD_SCHEMA_VERSION: &str = "vox_dogfood_v1";

#[allow(clippy::too_many_arguments)]
pub async fn run(
    analyze: bool,
    frequency: bool,
    time: bool,
    errors: bool,
    workflows: bool,
    apply_prefs: bool,
    feedback: bool,
    export: bool,
    export_dataset: Option<Option<std::path::PathBuf>>,
    record_eval: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    let db = VoxDb::connect_default().await?;
    let learner = db.learner();
    let user_id = vox_gamify::util::DEFAULT_USER_ID;

    if let Some(path) = record_eval {
        let json_str = std::fs::read_to_string(&path)?;
        let v: serde_json::Value = serde_json::from_str(&json_str)?;
        let run_id = v
            .get("run_id")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown");
        let model_path = v.get("model_path").and_then(|x| x.as_str());
        let format_validity = v.get("format_validity").and_then(|x| x.as_f64());
        let safety_rejection_rate = v.get("safety_rejection_rate").and_then(|x| x.as_f64());
        let quality_proxy = v.get("quality_proxy").and_then(|x| x.as_f64());
        let metadata_json = v.get("metadata_json").and_then(|x| x.as_str());
        let id = db
            .record_eval_run(
                run_id,
                model_path,
                format_validity,
                safety_rejection_rate,
                quality_proxy,
                metadata_json,
            )
            .await?;
        println!("Recorded eval run {} (id {})", run_id, id);
        return Ok(());
    }

    if export_dataset.is_some() {
        let dir = export_dataset
            .flatten()
            .unwrap_or_else(|| std::path::PathBuf::from("target/dogfood"));
        export_dataset_to_dir(&learner, &dir).await?;
        return Ok(());
    }

    println!("{}", "Vox Behavioral Learning Engine".bold().cyan());
    println!("{}", "==============================".cyan());

    let show_analyze = analyze
        || (!frequency && !time && !errors && !workflows && !apply_prefs && !feedback && !export);

    if show_analyze {
        println!("\n{}", "--- Detected Patterns ---".bold().yellow());
        let patterns = learner.analyze(user_id).await?;
        if patterns.is_empty() {
            println!("No patterns detected yet. Keep using Vox to build a history.");
        } else {
            for p in patterns {
                println!(
                    "[{}] {} (confidence: {:.0}%)",
                    p.category.green(),
                    p.description,
                    p.confidence * 100.0
                );
            }
        }

        let suggestions = learner.suggest(user_id).await?;
        if !suggestions.is_empty() {
            println!("\n{}", "--- Suggestions ---".bold().green());
            for s in suggestions {
                println!("• {}: {}", s.title.bold(), s.description);
                if let Some(action) = s.action {
                    println!("  Action: {}", action.dimmed());
                }
            }
        }
    }

    if frequency {
        println!("\n{}", "--- Command Frequency ---".bold().yellow());
        let items = learner.frequency_analysis(user_id, 20).await?;
        if items.is_empty() {
            println!("No command usage recorded yet.");
        } else {
            println!(
                "{:<15} {:<10} {:<10} {:<10}",
                "Command", "Count", "Success", "Avg Dur(ms)"
            );
            for item in items {
                println!(
                    "{:<15} {:<10} {:<10.1}% {:<10.0}",
                    item.item.cyan(),
                    item.count,
                    item.success_rate * 100.0,
                    item.avg_duration_ms.unwrap_or(0.0)
                );
            }
        }
    }

    if time {
        println!("\n{}", "--- Usage by Hour ---".bold().yellow());
        let buckets = learner.time_analysis(user_id).await?;
        if buckets.is_empty() {
            println!("No time-of-day data available yet.");
        } else {
            for b in buckets {
                let bar = "█".repeat((b.count as usize / 5).max(1));
                println!("{:02}:00 {:<30} ({})", b.hour, bar.cyan(), b.count);
            }
        }
    }

    if errors {
        println!("\n{}", "--- Common Error Patterns ---".bold().red());
        let errs = learner.error_analysis(user_id).await?;
        if errs.is_empty() {
            println!("No error patterns detected — great work!");
        } else {
            for e in errs {
                println!("{} - {} occurrences", e.error_type.bold().red(), e.count);
                if let Some(ctx) = e.recent_context {
                    println!("  Last context: {}", ctx.dimmed());
                }
            }
        }
    }

    if workflows {
        println!("\n{}", "--- Workflow Sequences ---".bold().yellow());
        let seqs = learner.workflow_analysis(user_id).await?;
        if seqs.is_empty() {
            println!("No workflow sequences detected yet. Keep using Vox to build patterns.");
        } else {
            for s in &seqs {
                if s.actions.len() >= 2 {
                    println!(
                        "{} -> {} ({} times)",
                        s.actions[0].cyan(),
                        s.actions[1].cyan(),
                        s.frequency
                    );
                }
            }
        }
    }

    if apply_prefs {
        println!("\n{}", "--- Inferring Preferences ---".bold().yellow());
        let prefs = learner.preference_inference(user_id).await?;
        if prefs.is_empty() {
            println!("No confident preferences inferred yet.");
        } else {
            for (key, val) in prefs {
                println!("Set {} = {}", key.green(), val);
            }
        }
    }

    if feedback {
        println!("\n{}", "--- Processing Feedback Loop ---".bold().yellow());
        let updates = learner.feedback_loop(user_id).await?;
        if updates == 0 {
            println!("No feedback data to process.");
        } else {
            println!(
                "Processed {} feedback entries into learning patterns.",
                updates.to_string().green()
            );
        }
    }

    if export {
        println!("\n{}", "--- Training Data Export ---".bold().yellow());
        let data = learner.export_training_data(100).await?;
        if data.is_empty() {
            println!("No training data available for export.");
        } else {
            println!("Exported {} training pairs:", data.len());
            for pair in &data {
                let prompt_preview: String = pair.prompt.chars().take(60).collect();
                let response_preview: String = pair.response.chars().take(40).collect();
                println!(
                    "  {} → {} {}",
                    prompt_preview.dimmed(),
                    response_preview,
                    if let Some(r) = pair.rating {
                        format!("[{}★]", r).yellow().to_string()
                    } else {
                        "[unrated]".dimmed().to_string()
                    }
                );
            }
        }
    }

    Ok(())
}

async fn export_dataset_to_dir(
    learner: &vox_db::learning::BehavioralLearner<'_>,
    dir: &Path,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let pairs = learner.export_training_data(EXPORT_DATASET_LIMIT).await?;
    let train_path = dir.join("train.jsonl");
    let mut f = std::fs::File::create(&train_path)?;
    for pair in &pairs {
        let obj = serde_json::json!({
            "prompt": pair.prompt,
            "response": pair.response,
            "instruction": pair.prompt,
            "output": pair.response,
            "rating": pair.rating,
            "feedback_type": pair.feedback_type,
            "correction": pair.correction,
            "preferred": pair.preferred,
        });
        writeln!(f, "{}", serde_json::to_string(&obj)?)?;
    }
    f.sync_all()?;
    let exported_at = chrono::Utc::now().to_rfc3339();
    let metadata = serde_json::json!({
        "schema_version": DOGFOOD_SCHEMA_VERSION,
        "count": pairs.len(),
        "exported_at": exported_at,
        "limit": EXPORT_DATASET_LIMIT,
    });
    std::fs::write(
        dir.join("metadata.json"),
        serde_json::to_string_pretty(&metadata)?,
    )?;
    println!(
        "Exported {} pairs to {} (metadata.json, train.jsonl)",
        pairs.len(),
        dir.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use vox_db::DbConfig;

    #[tokio::test]
    async fn export_dataset_produces_valid_jsonl_and_metadata() {
        let db = vox_db::VoxDb::connect(DbConfig::Memory)
            .await
            .expect("memory db");
        let iid = db
            .store()
            .log_interaction(
                "test-session",
                Some("user1"),
                "What is Vox?",
                "Vox is an AI-native language.",
                "test-model",
                Some(100),
                Some(20),
            )
            .await
            .expect("log_interaction");
        db.store()
            .submit_feedback(iid, Some("user1"), Some(5), "rating", None, None)
            .await
            .expect("submit_feedback");

        let dir = std::env::temp_dir().join(format!(
            "vox_learn_export_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        export_dataset_to_dir(&db.learner(), &dir)
            .await
            .expect("export");

        let meta_path = dir.join("metadata.json");
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
        assert_eq!(
            meta.get("schema_version").and_then(|v| v.as_str()),
            Some(DOGFOOD_SCHEMA_VERSION)
        );
        assert_eq!(meta.get("count").and_then(|v| v.as_i64()), Some(1));

        let train_path = dir.join("train.jsonl");
        let lines: Vec<String> = std::io::BufReader::new(std::fs::File::open(&train_path).unwrap())
            .lines()
            .map(|r| r.unwrap())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(lines.len(), 1);
        let row: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(
            row.get("prompt").and_then(|v| v.as_str()),
            Some("What is Vox?")
        );
        assert_eq!(
            row.get("response").and_then(|v| v.as_str()),
            Some("Vox is an AI-native language.")
        );
        assert_eq!(row.get("rating").and_then(|v| v.as_i64()), Some(5));
        assert_eq!(
            row.get("feedback_type").and_then(|v| v.as_str()),
            Some("rating")
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
