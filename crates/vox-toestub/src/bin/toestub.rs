use std::path::PathBuf;
use vox_toestub::{ToestubConfig, ToestubEngine, OutputFormat};
use vox_toestub::rules::Severity;

fn main() -> anyhow::Result<()> {
    let config = ToestubConfig {
        roots: vec![PathBuf::from(".")],
        min_severity: Severity::Warning,
        format: OutputFormat::Terminal,
        suggest_fixes: true,
        ..Default::default()
    };

    let engine = ToestubEngine::new(config);
    let (result, output) = engine.run_and_report();

    println!("{}", output);

    if result.has_errors() {
        std::process::exit(1);
    }

    Ok(())
}
