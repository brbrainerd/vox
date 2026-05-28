//! Publication plugin for Vox.
//!
//! Consolidates publication concerns as an opt-in plugin:
//! - Ingest: RSS/Atom feed crawling (`vox-scientia-ingest::FeedCrawler`) with
//!   semantic deduplication (`IngestDeduplicator`).
//! - Publish: Reddit, YouTube, and scholarly job feeds via `vox-publisher`
//!   (features: `scientia-reddit`, `scientia-youtube`, `scholarly-external-jobs`).
//!
//! Default `vox-cli` builds do not include this plugin or its heavy transitive
//! deps (`feed-rs`, `fnv`, platform HTTP clients). Users opt in via:
//!   `vox plugin install publication`

use abi_stable::{
    erased_types::TD_Opaque, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::extensions::publication::{Publication, Publication_TO};
use vox_plugin_api::host::VoxHost_TO;

// Re-export public surface from dependent libraries so hosts can reach them
// without depending on those crates directly.
pub use vox_scientia::ingest::{FeedCrawler, FeedSource, InboundItem, IngestDeduplicator};

pub mod ingest;
pub use ingest::ingest_tick;

#[export_root_module]
fn root_module() -> VoxPluginRootRef {
    VoxPluginRoot {
        abi_version: VOX_PLUGIN_ABI_VERSION,
        manifest_json,
        init,
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn manifest_json() -> RString {
    RString::from(r#"{"id":"publication","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    tracing::info!("vox-plugin-publication loaded (RSS ingest + Reddit/YouTube/scholarly publish)");
    let plugin = PublicationPlugin;
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}

#[derive(Clone)]
struct PublicationPlugin;

impl VoxPlugin for PublicationPlugin {
    fn id(&self) -> RString {
        RString::from("publication")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }

    fn as_publication(&self) -> ROption<Publication_TO<'static, RBox<()>>> {
        ROption::RSome(Publication_TO::from_value(self.clone(), TD_Opaque))
    }
}

impl Publication for PublicationPlugin {
    fn ingest_tick(&self, feed_id: ROption<RString>, limit: u32) -> RResult<(), RBoxError> {
        // The underlying `ingest::ingest_tick` is async (uses tokio'd DB
        // and HTTP), but `#[sabi_trait]` methods are sync over the FFI
        // boundary. Same pattern as `vox-plugin-populi-mesh`: spin up a
        // small per-call current-thread runtime and `block_on` inside it.
        // Callers (`vox-cli`) wrap this in `tokio::task::spawn_blocking`
        // so the outer multi-thread runtime is not blocked.
        let mini_rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                return RResult::RErr(RBoxError::new(e));
            }
        };
        let feed_id_owned: Option<String> = feed_id.into_option().map(|s| s.into_string());
        let limit_usize = limit as usize;
        let result = mini_rt.block_on(async move {
            ingest::ingest_tick(feed_id_owned.as_deref(), limit_usize).await
        });
        match result {
            Ok(()) => RResult::ROk(()),
            Err(e) => RResult::RErr(RBoxError::from_box(Box::<
                dyn std::error::Error + Send + Sync,
            >::from(e.to_string()))),
        }
    }
}
