# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- README rewritten to match the current crate identity (`oxideav-meta`,
  not `oxideav-format-all`) and the current API (`register_all(ctx)` +
  `populate_mesh3d_registry(reg)`, not the linkme-era
  `RuntimeContext::with_all_features()` / `with_all_features_filtered()`
  that no longer exist on `oxideav-core`). Documents the build-script
  flow end-to-end and the preset-bundle table.

### Added

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
