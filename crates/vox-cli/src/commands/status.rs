//! `vox status` — AI provider usage, remaining budget, and cost summary.

use anyhow::Result;
use serde_json::json;

const PROVIDER_LIMITS: &[(&str, &str, u32)] = &[
    ("google", "gemini-2.0-flash-lite", 1000),
    ("google", "gemini-2.5-flash-preview", 250),
    ("google", "gemini-2.5-pro", 100),
    ("openrouter", ":free models", 50),
    ("ollama", "local models", u32::MAX),
];

pub async fn run(json_output: bool) -> Result<()> {
    // Detect configured providers
    let google_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_AI_STUDIO_KEY"))
        .ok()
        .or_else(|| super::login::get_auth("google").map(|a| a.token));

    let openrouter_key = std::env::var("OPENROUTER_API_KEY")
        .ok()
        .or_else(|| super::login::get_auth("openrouter").map(|a| a.token));

    let ollama_up = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 11434)),
        std::time::Duration::from_millis(300),
    )
    .is_ok();

    // Try VoxDB for real counters
    let db = vox_db::VoxDb::connect_default().await.ok();

    if json_output {
        let mut providers = Vec::new();
        for (prov, model, limit) in PROVIDER_LIMITS {
            let configured = match *prov {
                "google" => google_key.is_some(),
                "openrouter" => openrouter_key.is_some(),
                "ollama" => ollama_up,
                _ => false,
            };
            let calls_used = if let Some(ref db) = db {
                let tracker = vox_orchestrator::usage::UsageTracker::new_ref(db);
                tracker.get_calls_today(prov, None).await.unwrap_or(0)
            } else {
                0
            };
            let remaining = if *limit == u32::MAX {
                -1i64
            } else {
                limit.saturating_sub(calls_used) as i64
            };
            providers.push(json!({
                "provider": prov,
                "model": model,
                "configured": configured,
                "calls_used": calls_used,
                "daily_limit": if *limit == u32::MAX { -1i64 } else { *limit as i64 },
                "remaining": remaining,
            }));
        }

        let cost = if let Some(ref db) = db {
            let tracker = vox_orchestrator::usage::UsageTracker::new_ref(db);
            tracker.cost_summary_today().await.map(|c| c.total_cost_usd).unwrap_or(0.0)
        } else {
            0.0
        };

        println!("{}", serde_json::to_string_pretty(&json!({
            "providers": providers,
            "cost_today_usd": cost,
        }))?);
        return Ok(());
    }

    // Human-readable table
    println!();
    println!(
        "  {:<12} {:<28} {:>7} {:>8} {:>11}  {}",
        "Provider", "Model", "Today", "Limit", "Remaining", "Status"
    );
    println!(
        "  {:<12} {:<28} {:>7} {:>8} {:>11}  {}",
        "──────────", "──────────────────────────", "─────", "─────", "─────────", "──────"
    );

    let mut grand_total = 0u32;
    let mut grand_cost = 0.0f64;

    for (prov, model, limit) in PROVIDER_LIMITS {
        let configured = match *prov {
            "google" => google_key.is_some(),
            "openrouter" => openrouter_key.is_some(),
            "ollama" => ollama_up,
            _ => false,
        };
        let calls_used = if let Some(ref db) = db {
            let tracker = vox_orchestrator::usage::UsageTracker::new_ref(db);
            tracker.get_calls_today(prov, None).await.unwrap_or(0)
        } else {
            0
        };
        grand_total += calls_used;

        let remaining_str = if *limit == u32::MAX {
            "∞".to_string()
        } else {
            format!("{}", limit.saturating_sub(calls_used))
        };
        let limit_str = if *limit == u32::MAX { "∞".to_string() } else { format!("{}", limit) };

        let status = if !configured {
            "\x1b[2mnot configured\x1b[0m"
        } else if *limit != u32::MAX && calls_used >= *limit {
            "\x1b[31mexhausted\x1b[0m"
        } else {
            "\x1b[32mhealthy\x1b[0m"
        };

        println!(
            "  {:<12} {:<28} {:>7} {:>8} {:>11}  {}",
            prov, model, calls_used, limit_str, remaining_str, status
        );
    }

    if let Some(ref db) = db {
        let tracker = vox_orchestrator::usage::UsageTracker::new_ref(db);
        grand_cost = tracker.cost_summary_today().await.map(|c| c.total_cost_usd).unwrap_or(0.0);
    }

    println!();
    println!("  Total today: {} calls · ${:.4} spent", grand_total, grand_cost);
    println!();

    Ok(())
}
