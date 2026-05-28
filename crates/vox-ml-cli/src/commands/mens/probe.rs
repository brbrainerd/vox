//! `vox mens probe` — GPU capability detection and recommended config.

use anyhow::Result;

pub async fn run_probe(verbose: bool) -> Result<()> {
    #[cfg(not(feature = "gpu"))]
    {
        let _ = verbose;
        anyhow::bail!("`vox mens probe` requires --features gpu");
    }
    #[cfg(feature = "gpu")]
    {
        use owo_colors::OwoColorize;
        use vox_populi::mens::hardware;
        use vox_populi::mens::tensor::device::recommend_config;

        // New SSOT: get summary and real-time telemetry
        let summary = hardware::HardwareRegistry::probe().await;
        let telemetry = hardware::HardwareRegistry::monitor();

        println!("{}", "--- GPU Discovery (SSOT) ---".bold().cyan());
        println!("Model:      {}", summary.model_name.green());
        println!("Vendor:     {:?}", summary.vendor);
        println!("VRAM:       {} MB", summary.vram_mb.yellow());
        println!("GPU Count:  {}", summary.gpu_count.yellow());
        println!("Backend:    {:?}", summary.backend);

        if let Some(t) = telemetry {
            println!("{}", "--- Real-time Telemetry ---".bold().blue());
            println!("Util:       {}%", t.utilization_pct);
            println!("Temp:       {}°C", t.temperature_c);
            println!("Power:      {}W", t.power_usage_w);
            println!("Used VRAM:  {} MB", t.memory_used_mb);

            // Record to DB if telemetry is enabled
            let repository_id =
                vox_repository::discover_repository_or_fallback(std::path::Path::new("."))
                    .repository_id;
            let node_id = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId)
                .expose()
                .map(|s| s.trim().to_string());
            let tel_json = serde_json::to_value(&t).unwrap();

            vox_db::populi_registry_telemetry::record_hardware_telemetry_opt(
                &repository_id,
                node_id.as_deref(),
                &tel_json,
            )
            .await;
        }

        if verbose {
            let profile = recommend_config(summary.vram_mb);
            println!();
            println!(
                "{}",
                format!(
                    "Recommended config for this hardware ({} profile):",
                    profile.label
                )
                .bold()
                .magenta()
            );
            println!(
                "  --rank {} --batch-size {} --seq-len {}",
                profile.suggested_rank, profile.suggested_batch, profile.max_seq_len
            );
            println!();
            println!("  Example training command:");
            println!(
                "    {} mens train --device {} --rank {} --batch-size {} --seq-len {}",
                "vox".cyan(),
                summary.backend.as_cli_flag(),
                profile.suggested_rank,
                profile.suggested_batch,
                profile.max_seq_len,
            );
        }

        Ok(())
    } // end #[cfg(feature = "gpu")]
}
