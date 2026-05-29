//! Smoke test for the build-script generated [`register_all`] helper.
//!
//! Runs against the default feature set (which is `["all"]`, pulling
//! every sibling codec/container/filter/source). The assertions are
//! deliberately weak — they only verify:
//!
//! 1. `register_all(&mut ctx)` is a real, callable fn that doesn't
//!    panic when invoked on a fresh `RuntimeContext`. (Confirms the
//!    build-script-generated `include!()` compiled and the wrapper-fn
//!    dispatch contract resolved every sibling's `__oxideav_entry`.)
//! 2. Once `register_all` returns, at least ONE of the four
//!    sub-registries (codecs, containers, filters, sources) has at
//!    least one entry. This guards against regressions where the
//!    build script silently emits an empty `register_all` body — the
//!    failure mode the original `ensure_linked()` workaround was
//!    meant to fight.
//!
//! Deliberately NOT asserted: which specific codecs / containers /
//! schemes are present, or their count. Those are downstream tests'
//! responsibility; this crate's job is just to demonstrate that the
//! dispatch shell works end-to-end.
//!
//! With non-default feature subsets (e.g. `default-features = false,
//! features = ["3d"]`) `register_all` may legitimately be empty —
//! every dep in that subset is in the build-script `SKIP` list. The
//! `default = ["all"]` build is the only one where the populate
//! assertion has to hold.

use oxideav_core::RuntimeContext;

#[test]
fn register_all_does_not_panic() {
    let mut ctx = RuntimeContext::new();
    oxideav_meta::register_all(&mut ctx);
}

#[test]
fn register_all_populates_at_least_one_sub_registry() {
    let mut ctx = RuntimeContext::new();
    oxideav_meta::register_all(&mut ctx);

    let has_codec =
        ctx.codecs.decoder_ids().next().is_some() || ctx.codecs.encoder_ids().next().is_some();
    let has_container = ctx.containers.demuxer_names().next().is_some()
        || ctx.containers.muxer_names().next().is_some();
    let has_source = ctx.sources.schemes().next().is_some();

    // At least one populated registry. The `all` default features set
    // should produce many; this assertion only requires one so the
    // test stays robust to per-sibling registrar landings/removals.
    // `FilterRegistry` has no public iter/len API, so we don't check
    // it here — the codec/container/source surface is broad enough to
    // catch a regression where the build-script silently emitted an
    // empty `register_all`.
    assert!(
        has_codec || has_container || has_source,
        "register_all populated no sub-registry; expected at least one entry across codecs/containers/sources",
    );
}

#[cfg(feature = "mesh3d")]
#[test]
fn populate_mesh3d_registry_does_not_panic() {
    let mut reg = oxideav_mesh3d::Mesh3DRegistry::new();
    oxideav_meta::populate_mesh3d_registry(&mut reg);
}
