# oxideav-meta

Aggregator crate for the [oxideav](https://github.com/OxideAV/oxideav) framework.

## What it is

`oxideav-meta` ships **no codec source code**. Its job is to depend on every oxideav sibling codec / container / filter / source / 3D-format crate the framework knows about, and expose two thin helpers that fold every enabled sibling's registrar into the consumer's [`oxideav_core::RuntimeContext`].

The build script (`build.rs`) parses this crate's own `Cargo.toml` plus the active `CARGO_FEATURE_*` env vars and emits a `register_all(ctx)` function whose body is one explicit `<sibling>::__oxideav_entry(ctx)` call per enabled sibling. Each call is what the sibling's `oxideav_core::register!` macro expanded to at the sibling's call site — wrapper-fn dispatch, no `linkme` distributed slice, no `#[used]` ctor / init_array tricks, no `ensure_linked()` workaround. Explicit fn calls force the linker to keep every enabled rlib alive on every target Rust supports (wasm included).

A parallel helper `populate_mesh3d_registry(reg)` (gated behind the `mesh3d` cargo feature) folds every enabled 3D-format sibling into an `oxideav_mesh3d::Mesh3DRegistry` — the 3D-format crates use a separate dispatch contract from the codec/container/filter/source path.

## Use it when

- You want **everything** the framework supports — bundle it into your tool / player / CLI / transcoder and have every codec available without listing each sibling by hand.
- You don't care about binary size and can afford every sibling's compile cost.

## Skip it when

- You only need a specific subset (e.g. just MP4 + H.264 + AAC for a video player). Depend on the individual sibling crates directly and call each one's `register(&mut ctx)` yourself, or pick a slimmer feature set on this crate (see "Slimming the build" below).
- You're targeting `no_std`. Both helpers take `&mut oxideav_core::RuntimeContext`, which requires `alloc`.

## Quick start

```toml
[dependencies]
oxideav-core = "0.1"
oxideav-meta = "*"
```

```ignore
use oxideav_core::RuntimeContext;

let mut ctx = RuntimeContext::new();
oxideav_meta::register_all(&mut ctx);
// `ctx` now contains every codec / container / filter / source
// enabled at build time (default-features = ["all"] pulls all of
// them).
```

## Slimming the build

Default features pull every sibling. Disable defaults and pick a coherent subset using preset bundles, or list individual crates:

```toml
[dependencies]
# Image codecs + image filter only.
oxideav-meta = { version = "*", default-features = false, features = ["image"] }

# Just MP4 + H.264 + AAC.
oxideav-meta = { version = "*", default-features = false, features = ["mp4", "h264", "aac"] }
```

Preset bundles:

| Preset           | Pulls                                                                                  |
| ---------------- | -------------------------------------------------------------------------------------- |
| `all` (default)  | Every sibling — codec, container, filter, source, 3D, hardware accel.                  |
| `pure-rust`      | `all` minus `hwaccel` — for builds that want zero FFI to OS HW-engine APIs.            |
| `audio`          | Every audio codec + `audio-filter` + the containers that commonly carry audio.         |
| `video`          | Every video codec + the containers that commonly carry video.                          |
| `image`          | Every still / animated image codec + `image-filter`.                                   |
| `subtitles`      | `ass`, `sub-image`, `subtitle`.                                                        |
| `3d`             | `mesh3d` (typed Scene3D model) + the `stl` / `obj` / `gltf` / `usdz` / `fbx` formats.  |
| `hwaccel`        | macOS `audiotoolbox` / `videotoolbox` + linux `vaapi` / `vdpau` / `nvidia` + `vulkan-video` (linux + windows). |
| `source-drivers` | `source` (file://) + `http` + `generator` (synth URIs) + `bluray` + `dvd`.             |

Per-crate features are named after each crate's short name (`aac`, `h264`, `mp4`, …). One name exception: `oxideav-mod` is feature `amiga-mod` to avoid the `mod` Rust keyword in feature lists.

Hardware-accel features are target-portable — enabling `vaapi` on macOS, or `videotoolbox` on linux, is a no-op (the underlying sibling dep only resolves on its supported target, so the feature degrades cleanly).

The standalone `vfw` feature (Windows-codec delegation bridge) is intentionally *not* part of any preset bundle. Enabling it makes `register_all` walk the discovery path (`OXIDEAV_VFW_CODEC_PATH` or `~/.local/share/oxideav/codecs/`) at registration time and probe each `*.dll` / `*.ax` it finds; that filesystem scan + cache lookup should be an explicit opt-in, not a side-effect of pulling `all`. Use `features = ["all", "vfw"]` (or `["pure-rust", "vfw"]`) when you want the bridge wired in.

## 3D scenes & assets

The `3d` preset enables `oxideav-mesh3d` (typed `Scene3D` / `Mesh` / `Material` / `Animation` model + `Mesh3DRegistry`) and the five format codec siblings (`oxideav-stl`, `oxideav-obj`, `oxideav-gltf`, `oxideav-usdz`, `oxideav-fbx`). They use a separate dispatch contract from the codec/container/filter/source path that [`register_all`] walks — call [`populate_mesh3d_registry`] to wire them into a registry:

```ignore
# #[cfg(feature = "mesh3d")] {
use oxideav_mesh3d::Mesh3DRegistry;

let mut reg = Mesh3DRegistry::new();
oxideav_meta::populate_mesh3d_registry(&mut reg);
// `reg` now resolves stl / obj / gltf / usdz / fbx by extension or
// format-id.
# }
```

## Introspection

Alongside `register_all` the build script emits two compile-time
constants so tooling can enumerate exactly which siblings meta wired
on the current target without having to peek inside `RuntimeContext`:

```ignore
// Every sibling whose `__oxideav_entry` is called by `register_all`
// on the current target — feature-filtered and target-gate-filtered.
for (crate_name, short) in oxideav_meta::ENABLED_SIBLINGS {
    println!("meta wires {crate_name} (short = {short})");
}

// Strict superset including macOS / linux / windows HW-accel entries
// whose target gate isn't satisfied on the current build. Useful for
// "what would meta know about if I were on linux?" tooling.
for (crate_name, short) in oxideav_meta::ENABLED_SIBLINGS_ALL {
    println!("known to meta: {crate_name}");
}
```

A parallel `ENABLED_MESH3D_FORMATS: &[&str]` (gated behind the
`mesh3d` feature) lists the 3D-format short names dispatched by
[`populate_mesh3d_registry`].

Both slices are alphabetised by crate name (the same order
`register_all` calls them in) and short names are guaranteed to equal
the crate name with `oxideav-` stripped.

## How the build script works

The build script is a 200-line Rust program in `build.rs`:

1. Reads this crate's own `Cargo.toml`.
2. Walks the `[dependencies]` table and the recognized `[target.'cfg(...)'.dependencies]` tables (macOS / linux / linux-or-windows) and collects every `oxideav-*` dep.
3. For each dep, checks whether its corresponding `CARGO_FEATURE_*` env var is set (cargo sets these for every enabled feature). If yes, emits a `oxideav_<short>::__oxideav_entry(ctx)` call into the generated `register_all`. When the dep was target-gated, the same `cfg(...)` body is emitted verbatim as a `#[cfg(...)]` attribute on the call so cross-target builds still produce working code.
4. Same logic for the five 3D-format crates → calls into `populate_mesh3d_registry`.
5. Emits two static-slice introspection constants — [`ENABLED_SIBLINGS`] (current-target view, with target gates evaluated against `CARGO_CFG_TARGET_OS`) and [`ENABLED_SIBLINGS_ALL`] (cross-target superset). When `mesh3d` is on, also emits `ENABLED_MESH3D_FORMATS`.
6. Writes the generated module to `$OUT_DIR/register_all.rs`; `src/lib.rs` pulls it in via `include!()`.

Adding a new sibling = add an optional dep line + a `name = ["dep:oxideav-name"]` feature line in `Cargo.toml`. The next build regenerates `register_all` automatically.

## License

MIT.
