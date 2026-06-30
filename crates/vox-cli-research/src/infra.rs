use owo_colors::OwoColorize;
use std::process::Command;

pub async fn up() -> anyhow::Result<()> {
    println!("{} Starting SearXNG sidecar...", "RUN".green().bold());
    let status = Command::new("docker")
        .args(["compose", "-f", "docker/searxng/compose.yml", "up", "-d"])
        .status()?;

    if status.success() {
        println!(
            "{} SearXNG started at http://localhost:8080",
            "SUCCESS".green().bold()
        );
        Ok(())
    } else {
        anyhow::bail!("Failed to start SearXNG sidecar. Is Docker running?")
    }
}

pub async fn down() -> anyhow::Result<()> {
    println!("{} Stopping SearXNG sidecar...", "RUN".yellow().bold());
    let _ = Command::new("docker")
        .args(["compose", "-f", "docker/searxng/compose.yml", "down"])
        .status()?;
    Ok(())
}

pub async fn status() -> anyhow::Result<()> {
    println!("{} Checking Research Backends...", "LOOKUP".blue().bold());

    // Check SearXNG
    let searxng_status = match reqwest::get("http://localhost:8080/healthz").await {
        Ok(res) if res.status().is_success() => "ONLINE".green().bold().to_string(),
        _ => "OFFLINE (requires 'vox research up')"
            .red()
            .bold()
            .to_string(),
    };
    println!("{:<15} {}", "SearXNG:", searxng_status);

    // Check DuckDuckGo (external)
    let ddg_status = match reqwest::get("https://duckduckgo.com").await {
        Ok(res) if res.status().is_success() => "ONLINE".green().bold().to_string(),
        _ => "UNREACHABLE".red().bold().to_string(),
    };
    println!("{:<15} {}", "DuckDuckGo:", ddg_status);

    // Check Tavily (optional)
    let tavily_res = vox_secrets::resolve_secret(vox_secrets::SecretId::TavilyApiKey);
    let tavily_key = tavily_res.expose();
    let tavily_status = if tavily_key.is_some() {
        "CONFIGURED (API Key Detected)".green().bold().to_string()
    } else {
        "NOT CONFIGURED (Optional)".yellow().bold().to_string()
    };
    println!("{:<15} {}", "Tavily:", tavily_status);

    Ok(())
}
