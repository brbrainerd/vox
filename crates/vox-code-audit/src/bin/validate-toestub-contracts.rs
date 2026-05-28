//! Fail-closed check: `contracts/toestub/suppression.v1.schema.json` parses as JSON,
//! ledger files validate against the schema, and `suppressions.v1.json` loads as a `SuppressionStore`.
//!
//! Run from the repository root: `cargo run -p vox-code-audit --bin validate-toestub-contracts`

fn main() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    vox_code_audit::suppression::validate_toestub_suppression_contracts(&root)?;
    println!(
        "OK: {}",
        root.join("contracts/toestub")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    Ok(())
}
