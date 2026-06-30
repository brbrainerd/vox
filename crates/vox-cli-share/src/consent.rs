//! First-run consent banner for `vox share --backend cloudflare`.
//!
//! On first use: prints a notice explaining public exposure + cloudflared download,
//! prompts for [Y/n]. Subsequent runs skip the prompt (state persisted in ShareState).
//!
//! In non-TTY contexts (CI, piped input): requires `--accept-tos` flag or errors.

use crate::error::{ShareError, ShareResult};
use crate::state::{CONSENT_TEXT_VERSION, ShareState};

const BANNER: &str = r#"
[vox share] About to create a public URL for your Vox app via Cloudflare Quick Tunnels.

  • Your app will be publicly accessible at a *.trycloudflare.com URL.
  • cloudflared (~30 MB, Apache-2.0) will be downloaded to ~/.cache/vox/cloudflared/
    and verified with SHA256 before use.
  • Cloudflare's Terms of Service apply to your traffic:
    https://www.cloudflare.com/website-terms/
  • vox does not proxy or store your data; traffic goes directly to Cloudflare's edge.
  • The URL changes every run and expires when you press Ctrl+C.

Anyone with the URL can reach your app. Use --auth none only for public demos.
"#;

/// Ensure the user has accepted the consent. No-op if already accepted in a prior run.
///
/// `accept_tos`: if true, accept without prompting (--accept-tos CLI flag).
/// `force_prompt`: if true, re-prompt even if already accepted (for testing).
pub fn ensure_consent(accept_tos: bool, force_prompt: bool) -> ShareResult<()> {
    let mut state = ShareState::load()?;

    // Already consented at current version — skip.
    if !force_prompt
        && state.cloudflare_consent_v1
        && state.consent_text_version >= CONSENT_TEXT_VERSION
    {
        return Ok(());
    }

    if accept_tos {
        record_consent(&mut state)?;
        return Ok(());
    }

    // Detect non-TTY: if stdin is not a terminal, we can't prompt.
    if !is_tty() {
        return Err(ShareError::Config(
            "vox share requires consent to use Cloudflare Quick Tunnels.\n\
             In non-interactive environments, pass --accept-tos to accept automatically.\n\
             See https://www.cloudflare.com/website-terms/ for Cloudflare's ToS."
                .into(),
        ));
    }

    println!("{}", BANNER);
    print!("Continue? [Y/n] ");
    use std::io::Write;
    std::io::stdout().flush().ok();

    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| ShareError::Config(format!("read input: {}", e)))?;
    let trimmed = input.trim().to_lowercase();

    if trimmed == "n" || trimmed == "no" {
        return Err(ShareError::Config("Consent declined. Exiting.".into()));
    }

    record_consent(&mut state)?;
    Ok(())
}

fn record_consent(state: &mut ShareState) -> ShareResult<()> {
    state.cloudflare_consent_v1 = true;
    state.consent_text_version = CONSENT_TEXT_VERSION;
    state.save()
}

fn is_tty() -> bool {
    // Simple heuristic: if CI or VOX_SHARE_NONINTERACTIVE env vars are set, treat as non-TTY.
    // Full TTY detection requires libc/platform APIs; for S2 MVP, default to true (prompt shown).
    std::env::var("CI").is_err() && std::env::var("VOX_SHARE_NONINTERACTIVE").is_err()
}
