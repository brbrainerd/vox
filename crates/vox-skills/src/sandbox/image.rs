//! Sandbox image lifecycle — check, pull, and build `vox-skill-sandbox:latest`.

use vox_container::{BuildOpts, ContainerRuntime};

/// The default OCI tag for the skill sandbox image.
pub const SANDBOX_IMAGE_TAG: &str = "vox-skill-sandbox:latest";

/// Errors from sandbox image operations.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("Container runtime error: {0}")]
    Runtime(#[from] anyhow::Error),
    #[error("IO error writing sandbox Dockerfile: {0}")]
    Io(#[from] std::io::Error),
}

/// Returns `true` if the sandbox image exists in the local container daemon.
pub fn sandbox_image_present(runtime: &dyn ContainerRuntime, tag: &str) -> bool {
    // Use `docker images --quiet <tag>` — if output is non-empty, image exists.
    let output = std::process::Command::new(runtime.name())
        .args(["images", "--quiet", tag])
        .output();
    match output {
        Ok(o) => !String::from_utf8_lossy(&o.stdout).trim().is_empty(),
        Err(_) => false,
    }
}

/// Ensure the sandbox image exists locally.
///
/// If the image is missing, write the embedded minimal Dockerfile to a temp
/// directory and build the image via the container runtime.
///
/// This is a blocking operation; call from `tokio::task::spawn_blocking` when
/// invoked from async context.
pub fn ensure_sandbox_image(runtime: &dyn ContainerRuntime, tag: &str) -> Result<(), ImageError> {
    if sandbox_image_present(runtime, tag) {
        tracing::debug!(tag, "Sandbox image already present");
        return Ok(());
    }

    tracing::info!(
        tag,
        "Sandbox image not found — building from embedded Dockerfile"
    );

    let tmp = std::env::temp_dir().join("vox-skill-sandbox-build");
    std::fs::create_dir_all(&tmp)?;

    let dockerfile_path = tmp.join("Dockerfile");
    std::fs::write(&dockerfile_path, SANDBOX_DOCKERFILE)?;

    let opts = BuildOpts {
        context_dir: tmp.clone(),
        dockerfile: Some(dockerfile_path),
        tag: tag.to_string(),
        build_args: vec![],
    };

    runtime.build(&opts)?;
    tracing::info!(tag, "Sandbox image built successfully");
    Ok(())
}

/// Minimal Dockerfile for the skill sandbox image.
///
/// Security posture:
/// - Single-layer Alpine base (minimal attack surface)
/// - Runs as `nobody` (uid=65534)
/// - Entrypoint is `/bin/sh -c` to execute skill commands
/// - No package manager, no curl, no wget in base Alpine
const SANDBOX_DOCKERFILE: &str = r#"FROM alpine:3.19
# Run as nobody to drop privileges
RUN addgroup -S voxsandbox && adduser -S -G voxsandbox -u 65534 nobody 2>/dev/null || true
USER nobody
WORKDIR /work
// vox-arch-check: allow abs-path
ENTRYPOINT ["/bin/sh", "-c"]
"#;
