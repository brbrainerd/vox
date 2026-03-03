use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize)]
struct OsvQuery {
    package: OsvPackage,
    version: String,
}

#[derive(Serialize)]
struct OsvPackage {
    name: String,
    ecosystem: String,
}

#[derive(Deserialize)]
struct OsvResponse {
    vulns: Option<Vec<Vulnerability>>,
}

#[derive(Deserialize)]
struct Vulnerability {
    id: String,
    summary: Option<String>,
}

/// `vox audit` — audit dependencies for known issues.
pub async fn run() -> Result<()> {
    let manifest_path = PathBuf::from("Vox.toml");
    let manifest = vox_pm::VoxManifest::load(&manifest_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .with_context(|| "No Vox.toml found. Run `vox init` first.")?;

    if manifest.dependencies.is_empty() {
        println!("No dependencies to audit.");
        return Ok(());
    }

    println!(
        "Auditing {} dependencies for {} using OSV database...\n",
        manifest.dependencies.len(),
        manifest.package.name
    );

    let mut issues = 0;
    let client = reqwest::Client::new();

    for (name, spec) in &manifest.dependencies {
        let ver = spec.version_req().unwrap_or("0.1.0");

        // Basic manifest checks
        if ver == "*" {
            println!("  ⚠ {name}: wildcard version `*` — pin to a specific range");
            issues += 1;
        }

        // Online OSV check
        match check_osv(&client, name, ver).await {
            Ok(Some(vulns)) => {
                for v in vulns {
                    println!(
                        "  ✗ {}: FOUND VULNERABILITY [{}]",
                        name.to_uppercase(),
                        v.id
                    );
                    if let Some(s) = v.summary {
                        println!("    Summary: {}", s);
                    }
                    issues += 1;
                }
            }
            Ok(None) => {
                // No vulnerabilities found
            }
            Err(e) => {
                println!("  ⚠ {name}: OSV check failed: {e}");
            }
        }
    }

    // Check lockfile exists
    let lock_path = PathBuf::from("vox.lock");
    if !lock_path.exists() {
        println!("  ⚠ No vox.lock found — run `vox install` to generate one");
        issues += 1;
    }

    if issues == 0 {
        println!("\n✓ No issues found. All dependencies look good!");
    } else {
        println!("\n⚠ Found {issues} issue(s). Review recommendations above.");
    }

    Ok(())
}

async fn check_osv(
    client: &reqwest::Client,
    name: &str,
    version: &str,
) -> Result<Option<Vec<Vulnerability>>> {
    let query = OsvQuery {
        package: OsvPackage {
            name: name.to_string(),
            ecosystem: "Vox".to_string(), // Or use "PyPI" if it's a python dep, etc.
        },
        version: version.replace('^', "").replace('~', ""), // Simple version normalization
    };

    let resp = client
        .post("https://api.osv.dev/v1/query")
        .json(&query)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let osv_resp: OsvResponse = resp.json().await?;
    Ok(osv_resp.vulns)
}
