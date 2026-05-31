//! `vox quantize` — quantize a local SafeTensors model with vox-quantize.
use std::path::PathBuf;

use vox_quantize::{quantize, DevicePref, QuantMixture};

#[derive(Debug, clap::Args)]
pub struct QuantizeArgs {
    /// Model directory (must contain config.json + *.safetensors).
    #[arg(long)]
    pub input: PathBuf,
    /// Output directory for the quantized artifact.
    #[arg(long)]
    pub output: PathBuf,
    /// Target mixture: q4_k_m | q5_k_m | q6_k | q8_0
    #[arg(long, default_value = "q4_k_m")]
    pub to: String,
    /// Skip the round-trip verification pass.
    #[arg(long, default_value_t = false)]
    pub no_verify: bool,
    /// Device: auto | cuda | metal | cpu (default auto → GPU when available).
    #[arg(long, default_value = "auto")]
    pub device: String,
    /// Emit the full report as JSON instead of a table.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

pub fn parse_mixture(s: &str) -> anyhow::Result<QuantMixture> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "q4_k_m" => QuantMixture::Q4KM,
        "q5_k_m" => QuantMixture::Q5KM,
        "q6_k" => QuantMixture::Q6K,
        "q8_0" => QuantMixture::Q8_0,
        other => {
            anyhow::bail!("unknown --to mixture `{other}` (expected q4_k_m|q5_k_m|q6_k|q8_0)")
        }
    })
}

pub fn parse_device(s: &str) -> anyhow::Result<DevicePref> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "auto" => DevicePref::Auto,
        "cuda" | "cuda:0" => DevicePref::Cuda(0),
        "metal" => DevicePref::Metal,
        "cpu" => DevicePref::Cpu,
        other => anyhow::bail!("unknown --device `{other}` (expected auto|cuda|metal|cpu)"),
    })
}

pub fn run(args: QuantizeArgs) -> anyhow::Result<()> {
    if !args.input.join("config.json").exists() {
        anyhow::bail!(
            "no config.json in {} — not a model directory",
            args.input.display()
        );
    }
    let mixture = parse_mixture(&args.to)?;
    let device = parse_device(&args.device)?;
    let req = vox_quantize::QuantizeRequest {
        input_dir: args.input.clone(),
        output_dir: args.output.clone(),
        mixture,
        verify: !args.no_verify,
        device,
    };
    let report = quantize(&req)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!(
        "{:<60} {:>8} {:>12} {:>10}",
        "tensor", "dtype", "params", "mse"
    );
    for s in &report.tensors {
        let note = if s.fallback { " (fallback)" } else { "" };
        println!(
            "{:<60} {:>8} {:>12} {:>10.2e}{}",
            s.name, s.target_dtype, s.params, s.mse, note
        );
    }
    let gib = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
    println!(
        "\n{:.2} GiB -> {:.2} GiB  ({:.2}x)   worst MSE {:.2e}",
        gib(report.total_src_bytes),
        gib(report.total_quant_bytes),
        report.compression_ratio,
        report.worst_mse,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mixture_maps_known_values() {
        assert!(matches!(parse_mixture("q4_k_m").unwrap(), QuantMixture::Q4KM));
        assert!(matches!(parse_mixture("Q8_0").unwrap(), QuantMixture::Q8_0));
        assert!(parse_mixture("bogus").is_err());
    }

    #[test]
    fn parse_device_maps_known_values() {
        assert!(matches!(parse_device("auto").unwrap(), DevicePref::Auto));
        assert!(matches!(parse_device("cuda").unwrap(), DevicePref::Cuda(0)));
        assert!(matches!(parse_device("cpu").unwrap(), DevicePref::Cpu));
        assert!(parse_device("gpu").is_err());
    }
}
