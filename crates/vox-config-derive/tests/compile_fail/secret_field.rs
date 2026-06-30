#[derive(Default, vox_config_derive::VoxConfig)]
#[vox_config(prefix = "VOX_X", group = "General")]
struct Bad {
    #[config(secret, default = "")]
    api_key: String,
}
fn main() {}
