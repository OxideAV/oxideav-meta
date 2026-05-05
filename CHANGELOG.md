# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
