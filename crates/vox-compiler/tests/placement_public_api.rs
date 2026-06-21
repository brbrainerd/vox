use vox_compiler::{Placement, PlacementMap};

#[test]
fn placement_types_are_public() {
    let _ = Placement::Shared;
    fn _takes(_m: &PlacementMap) {}
}
