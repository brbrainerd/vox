//! Vox plugin: oratio
//!
//! Provides the AudioCapture and SpeechToText extension points for the Oratio
//! speech-to-code pipeline. The SpeechToText impl uses the Candle Whisper backend
//! extracted from vox-speech (Unit 4 of the vox-populi extraction follow-up plan).

mod audio;
mod backends;
mod oratio_internals;

// Dylib export glue, stamped with the current ABI version. `init` delegates to the
// host-aware constructor `audio::make_plugin`. Byte-identical to the previous hand-written
// block; the `oratio` id is retained (the crate is vox-plugin-speech).
vox_plugin_sdk::declare_plugin! {
    id: "oratio",
    version: "0.1.0",
    init: |host| audio::make_plugin(host),
}
