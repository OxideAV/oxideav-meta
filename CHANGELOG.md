# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Categorisation surface for `ENABLED_SIBLINGS`:
  - `ENABLED_SIBLINGS_BY_CATEGORY: &[(&str, &[&str])]` — every entry
    in `ENABLED_SIBLINGS` grouped under a stable category label
    (`audio-codec` / `video-codec` / `image-codec` / `audio-filter` /
    `image-filter` / `container` / `subtitle` / `source` / `hwaccel` /
    `delegation` / `render`). Category order is fixed across builds;
    empty categories are omitted, so a slim feature set yields a
    compact slice. Within a section, short names inherit the
    alphabetical sort already imposed on `ENABLED_SIBLINGS`.
  - `const fn category_of(short: &str) -> Option<&'static str>` —
    feature-state-independent lookup from short name to category
    label. Returns `None` for the 3D-format crates in `build.rs`'s
    `SKIP` list (`mesh3d`, `stl`, `obj`, `gltf`, `usdz`, `fbx` — they
    route through `populate_mesh3d_registry` instead) and any unknown
    string. `const fn` so callers can fold it into `const` lookups
    and `static` initialisers.
  - Source-of-truth `CATEGORY_TABLE` in `build.rs` plus a
    `category_of_known_short` helper that panics at build time if any
    enabled sibling lacks a category row — adding a new dep to
    `Cargo.toml` without updating the table fails loud instead of
    silently disappearing from the slice.
- Six new assertions in `tests/register_all_smoke.rs` covering the
  categorisation surface: section order matches the documented stable
  order, each section is sorted internally, the flat union across
  sections equals `ENABLED_SIBLINGS`, sections are pairwise disjoint,
  `category_of(short)` round-trips with the slice, `category_of`
  returns `None` for the SKIP-list crates and arbitrary unknowns, and
  `category_of` is usable in `const` context.
- `populate_render_registry(&mut oxideav_render::RenderRegistry)` —
  parallel to `populate_mesh3d_registry`. Gated on the `render`
  feature (already enabled via the `3d` preset bundle). Today
  populates `"scanline"`; future render backends (raycaster,
  path-tracer) will be registered here.
- Seven new sibling crates wired into `register_all`:
  - **Video**: `oxideav-cinepak` (Cinepak / CVID), `oxideav-huffyuv`
    (HuffYUV + FFVHuff), `oxideav-magicyuv` (MagicYUV),
    `oxideav-utvideo` (Ut Video) — all folded into the `video` preset
    bundle alongside the existing lossless / legacy video codecs.
  - **Audio**: `oxideav-shorten` (Shorten / .shn) — folded into the
    `audio` preset bundle.
  - **Source driver**: `oxideav-dvd` (`dvd://` URI handler — ISO 9660 +
    UDF 1.02 mount + VIDEO_TS walk) — folded into the `source-drivers`
    preset bundle alongside `bluray`.
  - **Delegation bridge**: `oxideav-vfw` (Windows-codec sandbox via
    `ud-emulator` 32-bit x86 + PE32 loader + Video for Windows host).
    Exposed as a standalone `vfw` feature; intentionally NOT part of
    any preset bundle so the discovery-path filesystem scan +
    `*.dll` / `*.ax` probing stays an explicit opt-in.
- Per-crate features for each: `cinepak`, `huffyuv`, `magicyuv`,
  `utvideo`, `shorten`, `dvd`, `vfw`. Each follows the existing
  `name = ["dep:oxideav-name"]` shape that `build.rs` consumes when
  emitting the call list.
- Build-script introspection constants: `ENABLED_SIBLINGS: &[(&str,
  &str)]` (alphabetised `(crate_name, short_name)` pairs for every
  sibling whose `__oxideav_entry` is dispatched by `register_all` on
  the current target) and `ENABLED_SIBLINGS_ALL` (cross-target
  superset that retains target-gated entries whose `#[cfg(...)]`
  doesn't match the current build). Plus a feature-gated
  `ENABLED_MESH3D_FORMATS: &[&str]` (under `cfg(feature = "mesh3d")`)
  with the 3D-format short names dispatched by
  `populate_mesh3d_registry`. Useful for diagnostics, CLI listings,
  and integration tests that need to know which sibling crates a
  given meta build wired in without inspecting `RuntimeContext`.
- `gate_matches_target` helper in `build.rs` that evaluates the
  three recognised `SECTIONS` gate shapes (`target_os = "macos"`,
  `target_os = "linux"`, `any(target_os = "linux", target_os =
  "windows")`) against `CARGO_CFG_TARGET_OS` so `ENABLED_SIBLINGS`
  reflects exactly the calls `register_all` makes on the current
  target — no manual cfg duplication on the consumer side.
- Six new assertions in `tests/register_all_smoke.rs` covering: no
  duplicate entries in `ENABLED_SIBLINGS`, alphabetical sort order,
  `short == crate_name.strip_prefix("oxideav-")`, `ENABLED_SIBLINGS`
  ⊆ `ENABLED_SIBLINGS_ALL`, `register_all` populating
  sub-registries implies `ENABLED_SIBLINGS` is non-empty (cross-check
  the generated module is internally consistent), and
  `ENABLED_MESH3D_FORMATS` ⊆ the hard-coded 3D-format table.

### Changed

- README rewritten to match the current crate identity (`oxideav-meta`,
  not `oxideav-format-all`) and the current API (`register_all(ctx)` +
  `populate_mesh3d_registry(reg)`, not the linkme-era
  `RuntimeContext::with_all_features()` / `with_all_features_filtered()`
  that no longer exist on `oxideav-core`). Documents the build-script
  flow end-to-end and the preset-bundle table.
- README adds an "Introspection" section that documents the new
  `ENABLED_SIBLINGS` / `ENABLED_SIBLINGS_ALL` / `ENABLED_MESH3D_FORMATS`
  constants with usage examples, and the build-script flow numbered
  list grows a step covering the static-slice emission.

### Added (earlier in [Unreleased])

- `tests/register_all_smoke.rs` — integration smoke test that
  (a) `register_all(&mut ctx)` is callable and doesn't panic on a fresh
  `RuntimeContext`, (b) under the default `all` feature set at least
  one of codecs / containers / sources is populated (guards against
  the build-script silently emitting an empty `register_all` body),
  and (c) under `mesh3d` the parallel `populate_mesh3d_registry` is
  also callable.

## [0.0.1](https://github.com/OxideAV/oxideav-meta/releases/tag/v0.0.1) - 2026-05-10

### Added

- add vfw bridge to hwaccel preset
- add `pure-rust` preset (= all minus hwaccel)

### Other

- Wire 5 new 3D-asset siblings + populate_mesh3d_registry helper
- Revert "features: add vfw bridge to hwaccel preset"
- emit __oxideav_entry calls (oxideav-core register! macro shape)
- wire four new Linux/Windows HW codec sibling crates into register_all
- rename to oxideav-meta: drop linkme, switch to explicit register_all(ctx) + Cargo features
- explain why ensure_linked() cannot be automated
- build.rs auto-generates FORCE_LINK from Cargo.toml — single source of truth
- FORCE_LINK + ensure_linked() — defeat linker DCE on production binaries ([#520](https://github.com/OxideAV/oxideav-meta/pull/520))
- Initial commit: virtual aggregator with deps on every sibling

### Added

- 3D scenes & assets sibling crates wired into the aggregator:
  `oxideav-mesh3d` (typed Scene3D / Mesh / Material PBR / Skin /
  Animation / Camera / Light / AudioEmitter model + `Mesh3DRegistry`)
  and the four format codecs `oxideav-stl`, `oxideav-obj`,
  `oxideav-gltf`, `oxideav-usdz`. Per-crate cargo features added:
  `mesh3d`, `stl`, `obj`, `gltf`, `usdz`. New preset bundle `3d`
  pulls all five; `3d` is now part of `all` and `pure-rust`. The
  format crates use a separate dispatch contract from the
  codec/container/filter/source `register_all` path — `build.rs`
  emits a parallel `populate_mesh3d_registry(&mut Mesh3DRegistry)`
  helper (gated `#[cfg(feature = "mesh3d")]`) that calls each enabled
  3D-format crate's `register(&mut Mesh3DRegistry)`.
- Linux + cross-platform HW codec sibling crates wired into the
  aggregator: `oxideav-vaapi`, `oxideav-vdpau`, `oxideav-nvidia` (all
  `target_os = "linux"`-only) and `oxideav-vulkan-video` (`target_os
  = "linux"` or `"windows"`). Per-crate cargo features added:
  `vaapi`, `vdpau`, `nvidia`, `vulkan-video`. The `hwaccel` preset
  bundle now includes all six HW backends (the two macOS-only ones
  plus these four).
- `build.rs` generalized to recognize three target-cfg dependency
  sections — `target_os = "macos"`, `target_os = "linux"`, and
  `any(target_os = "linux", target_os = "windows")` — via a
  `SECTIONS` lookup table. Each generated `register(ctx)` call gets
  a verbatim `#[cfg(...)]` attribute when its sibling lives under a
  target gate, so the cross-target build still produces working
  code.
- Initial scaffolding: virtual aggregator crate with deps on every oxideav
  sibling codec / container / filter / source crate. No source code; just
  Cargo dependencies. Linking this crate populates
  `oxideav_core::REGISTRARS` (linkme distributed slice) with every
  sibling's registrar entry.
- README documents the priority-0 hardware accel placement and the
  runtime opt-out via `RuntimeContext::with_all_features_filtered`.
