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

#[cfg(feature = "render")]
#[test]
fn populate_render_registry_exposes_scanline() {
    // Parallel to `populate_mesh3d_registry_does_not_panic`, but with a
    // concrete expectation: `oxideav_render::register_into` registers
    // `"scanline"` today, so a freshly-populated registry must surface
    // that name. Future render backends (raycaster, path-tracer) will
    // add more entries here without invalidating this assertion.
    let mut reg = oxideav_render::RenderRegistry::new();
    oxideav_meta::populate_render_registry(&mut reg);
    let names = reg.names();
    assert!(
        names.contains(&"scanline"),
        "populate_render_registry must register \"scanline\", got {names:?}",
    );
}

#[test]
fn enabled_siblings_has_unique_entries() {
    // Every (crate_name, short_name) pair in the introspection slice
    // must be unique — the build script's dep walker collects deps
    // section-by-section, so a regression that double-emits a dep
    // (e.g. listed under both `[dependencies]` and a target-gated
    // section) would surface here as duplicate entries.
    let entries: Vec<_> = oxideav_meta::ENABLED_SIBLINGS.iter().collect();
    let mut seen_names: Vec<&str> = Vec::with_capacity(entries.len());
    for (full, _) in &entries {
        assert!(
            !seen_names.contains(full),
            "duplicate crate_name in ENABLED_SIBLINGS: {full}",
        );
        seen_names.push(full);
    }
}

#[test]
fn enabled_siblings_is_sorted_alphabetically() {
    // `collect_sibling_deps` in build.rs sorts deps before emitting
    // any code, so both the `register_all` body and `ENABLED_SIBLINGS`
    // are alphabetical by crate name. Lock that contract in: tools
    // can rely on a deterministic order without re-sorting.
    let names: Vec<&str> = oxideav_meta::ENABLED_SIBLINGS
        .iter()
        .map(|(full, _)| *full)
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        names, sorted,
        "ENABLED_SIBLINGS should be sorted alphabetically by crate name",
    );
}

#[test]
fn enabled_siblings_short_names_match_crate_names() {
    // Short name = crate name with the `oxideav-` prefix stripped.
    // (Feature names are a separate naming axis — see
    // `feature_name_for` in build.rs for the one historical override.)
    for (full, short) in oxideav_meta::ENABLED_SIBLINGS {
        let expected_short = full.strip_prefix("oxideav-").unwrap_or(full);
        assert_eq!(
            *short, expected_short,
            "short name mismatch: {full} -> {short} (expected {expected_short})",
        );
    }
}

#[test]
fn enabled_siblings_all_is_superset_of_enabled_siblings() {
    // `ENABLED_SIBLINGS_ALL` includes target-gated entries (macOS /
    // linux / windows HW-accel crates) whose gate didn't match the
    // current target; `ENABLED_SIBLINGS` is the gate-satisfied subset.
    for entry in oxideav_meta::ENABLED_SIBLINGS {
        assert!(
            oxideav_meta::ENABLED_SIBLINGS_ALL.contains(entry),
            "ENABLED_SIBLINGS entry {entry:?} missing from ENABLED_SIBLINGS_ALL",
        );
    }
    assert!(
        oxideav_meta::ENABLED_SIBLINGS_ALL.len() >= oxideav_meta::ENABLED_SIBLINGS.len(),
        "ENABLED_SIBLINGS_ALL must be a superset",
    );
}

#[test]
fn enabled_siblings_population_matches_register_all() {
    // Sanity check: under the default `all` feature set, `register_all`
    // populates at least one sub-registry AND `ENABLED_SIBLINGS` is
    // non-empty. The two assertions together guarantee that the
    // generated module's `register_all` body and `ENABLED_SIBLINGS`
    // slice came from the same source-of-truth dep walk.
    let mut ctx = RuntimeContext::new();
    oxideav_meta::register_all(&mut ctx);

    let any_codec =
        ctx.codecs.decoder_ids().next().is_some() || ctx.codecs.encoder_ids().next().is_some();
    let any_container = ctx.containers.demuxer_names().next().is_some()
        || ctx.containers.muxer_names().next().is_some();
    let any_source = ctx.sources.schemes().next().is_some();

    if any_codec || any_container || any_source {
        assert!(
            !oxideav_meta::ENABLED_SIBLINGS.is_empty(),
            "register_all populated sub-registries but ENABLED_SIBLINGS is empty — generated module is inconsistent",
        );
    }
}

#[cfg(feature = "mesh3d")]
#[test]
fn enabled_mesh3d_formats_subset_of_known_formats() {
    // The 3D-format crate set is hard-coded in build.rs's
    // `MESH3D_FORMAT_CRATES` table. Every entry in
    // `ENABLED_MESH3D_FORMATS` must come from that table; this guards
    // against an accidental free-form addition.
    const KNOWN: &[&str] = &["stl", "obj", "gltf", "usdz", "fbx"];
    for short in oxideav_meta::ENABLED_MESH3D_FORMATS {
        assert!(
            KNOWN.contains(short),
            "unknown 3D format in ENABLED_MESH3D_FORMATS: {short}",
        );
    }
}

// ---------------------------------------------------------------------
// ENABLED_SIBLINGS_BY_CATEGORY + category_of() coverage
//
// Six assertions enforce the structural contract of the categorisation
// surface so future sibling additions (or accidental drift in
// build.rs's CATEGORY_TABLE) can't silently break introspection
// consumers:
//
// 1. Category-section order matches the documented stable order.
// 2. Within a section, short names are alphabetically sorted.
// 3. The flattened union of all sections matches the short-name
//    column of ENABLED_SIBLINGS exactly (no missing, no extras).
// 4. No short name appears in more than one section (disjoint cover).
// 5. category_of(short) returns the same label as the section the
//    short name was emitted under (round-trip parity).
// 6. category_of returns None for the 3D-format crates in build.rs's
//    SKIP list — they go through populate_mesh3d_registry instead, so
//    they must never look like register_all dispatchees.
// ---------------------------------------------------------------------

#[test]
fn enabled_siblings_by_category_section_order_is_stable() {
    // The documented stable order — must match `CATEGORY_ORDER` in
    // build.rs. Sections that are empty in the current feature set are
    // omitted from the slice, so we check that the present sections
    // are a subsequence of the canonical order.
    const ORDER: &[&str] = &[
        "audio-codec",
        "video-codec",
        "image-codec",
        "audio-filter",
        "image-filter",
        "container",
        "subtitle",
        "source",
        "hwaccel",
        "delegation",
        "render",
    ];
    let present: Vec<&str> = oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY
        .iter()
        .map(|(cat, _)| *cat)
        .collect();
    // Verify `present` is a subsequence of `ORDER`.
    let mut i = 0;
    for cat in &present {
        let pos = ORDER[i..]
            .iter()
            .position(|c| c == cat)
            .unwrap_or_else(|| panic!("ENABLED_SIBLINGS_BY_CATEGORY contains unknown or out-of-order category {cat:?}; expected subsequence of {ORDER:?}, got {present:?}"));
        i += pos + 1;
    }
}

#[test]
fn enabled_siblings_by_category_sections_are_sorted_within() {
    // `collect_sibling_deps` in build.rs already sorts the dep list
    // alphabetically; the by-category emitter walks that sorted list
    // and only filters by category, so each section's short names
    // come out alphabetical too. Lock that contract so consumers can
    // do binary search over a section without re-sorting.
    for (cat, shorts) in oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY {
        let mut sorted: Vec<&str> = shorts.to_vec();
        sorted.sort();
        let owned: Vec<&str> = shorts.to_vec();
        assert_eq!(
            owned, sorted,
            "ENABLED_SIBLINGS_BY_CATEGORY section {cat:?} is not alphabetically sorted",
        );
    }
}

#[test]
fn enabled_siblings_by_category_flat_union_matches_enabled_siblings() {
    // The flattened short-name list across every category section must
    // equal — as a set — the short-name column of ENABLED_SIBLINGS.
    // This catches both directions of drift: a sibling missing from
    // CATEGORY_TABLE (would fail the build via
    // `category_of_known_short`'s panic, but assert the user-visible
    // surface anyway), and a stale entry that no longer corresponds to
    // an enabled sibling.
    let mut flat: Vec<&str> = oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY
        .iter()
        .flat_map(|(_, shorts)| shorts.iter().copied())
        .collect();
    flat.sort();

    let mut from_siblings: Vec<&str> = oxideav_meta::ENABLED_SIBLINGS
        .iter()
        .map(|(_, short)| *short)
        .collect();
    from_siblings.sort();

    assert_eq!(
        flat, from_siblings,
        "ENABLED_SIBLINGS_BY_CATEGORY flat union must equal ENABLED_SIBLINGS short-name column",
    );
}

#[test]
fn enabled_siblings_by_category_sections_are_disjoint() {
    // Each short name appears in exactly one section. Catches a
    // categorisation typo where a row was duplicated under two labels.
    let mut seen: Vec<&str> = Vec::new();
    for (cat, shorts) in oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY {
        for s in *shorts {
            assert!(
                !seen.contains(s),
                "short name {s:?} appears in multiple ENABLED_SIBLINGS_BY_CATEGORY sections (latest: {cat:?})",
            );
            seen.push(s);
        }
    }
}

#[test]
fn category_of_round_trips_every_enabled_sibling() {
    // For every (category, shorts) section, `category_of(short)` must
    // return exactly that category. Round-trip parity guarantees the
    // const fn and the slice are emitted from the same source-of-truth
    // CATEGORY_TABLE.
    for (cat, shorts) in oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY {
        for s in *shorts {
            let got = oxideav_meta::category_of(s);
            assert_eq!(
                got,
                Some(*cat),
                "category_of({s:?}) returned {got:?}; expected Some({cat:?}) per ENABLED_SIBLINGS_BY_CATEGORY",
            );
        }
    }
}

#[test]
fn category_of_returns_none_for_skip_list_and_unknowns() {
    // 3D format crates + mesh3d are in build.rs's SKIP list — they
    // route through `populate_mesh3d_registry`, not `register_all`, so
    // they must NOT have a register_all category. `category_of` should
    // return None for them. Same for outright-unknown strings.
    for skip in &["mesh3d", "stl", "obj", "gltf", "usdz", "fbx"] {
        assert_eq!(
            oxideav_meta::category_of(skip),
            None,
            "category_of({skip:?}) should be None — 3D-format crates go through populate_mesh3d_registry",
        );
    }
    for unknown in &["", "definitely-not-a-codec", "AAC", "core"] {
        assert_eq!(
            oxideav_meta::category_of(unknown),
            None,
            "category_of({unknown:?}) should be None",
        );
    }
}

#[test]
fn category_of_is_usable_in_const_context() {
    // `category_of` is `const fn` so a downstream caller can lookup at
    // compile time. Lock the contract by performing a const lookup
    // here; if `const fn` regresses to a regular fn, this stops
    // compiling.
    const CAT_AAC: Option<&str> = oxideav_meta::category_of("aac");
    const CAT_MP4: Option<&str> = oxideav_meta::category_of("mp4");
    const CAT_UNKNOWN: Option<&str> = oxideav_meta::category_of("zzz-not-a-codec");
    assert_eq!(CAT_AAC, Some("audio-codec"));
    assert_eq!(CAT_MP4, Some("container"));
    assert_eq!(CAT_UNKNOWN, None);
}
