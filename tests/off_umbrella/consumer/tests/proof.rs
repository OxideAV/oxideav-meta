//! The off-umbrella proof: consumed from a packaged `.crate` with the
//! normalized manifest cargo ships, `oxideav-meta` must still wire
//! every enabled sibling. The published 0.0.2 failed both tests here
//! (`ENABLED_SIBLINGS` was empty, `register_all` installed nothing)
//! because `build.rs` only understood the in-tree manifest shape.

use oxideav_core::CodecId;
use oxideav_meta_off_umbrella_proof::registered_context;

#[test]
fn enabled_siblings_lists_the_requested_features() {
    eprintln!("ENABLED_SIBLINGS = {:?}", oxideav_meta::ENABLED_SIBLINGS);
    let expected: &[(&str, &str)] = &[
        ("oxideav-farbfeld", "farbfeld"),
        ("oxideav-png", "png"),
        ("oxideav-qoi", "qoi"),
    ];
    assert_eq!(
        oxideav_meta::ENABLED_SIBLINGS,
        expected,
        "build.rs did not pick the enabled siblings out of the packaged manifest"
    );
    assert_eq!(oxideav_meta::ENABLED_SIBLINGS_ALL, expected);
    assert_eq!(
        oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY,
        &[("image-codec", &["farbfeld", "png", "qoi"][..])]
    );
}

#[test]
fn register_all_resolves_png_outside_the_umbrella() {
    let ctx = registered_context();
    for id in ["png", "farbfeld", "qoi"] {
        assert!(
            ctx.codecs.has_decoder(&CodecId::new(id)),
            "register_all did not install the {id} decoder"
        );
    }
    assert_eq!(
        ctx.containers.container_for_extension("png"),
        Some("png"),
        "register_all did not install the png container"
    );
    assert!(
        ctx.containers.demuxer_names().any(|n| n == "png"),
        "png demuxer missing; demuxers = {:?}",
        ctx.containers.demuxer_names().collect::<Vec<_>>()
    );
}
