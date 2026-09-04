# oxideav-meta

[![CI](https://github.com/OxideAV/oxideav-meta/actions/workflows/ci.yml/badge.svg)](https://github.com/OxideAV/oxideav-meta/actions/workflows/ci.yml) [![crates.io](https://img.shields.io/crates/v/oxideav-meta.svg)](https://crates.io/crates/oxideav-meta) [![docs.rs](https://docs.rs/oxideav-meta/badge.svg)](https://docs.rs/oxideav-meta) [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

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

### Container / codec siblings wired individually

Every sibling is one feature; the table lists the ones whose wiring
is worth a note (bundle membership, library-only status, or an entry
point that deviates from the crate-root `__oxideav_entry` contract).

| Feature | Crate            | Registers                                                                   | Bundles           | Note                                                                                                   |
| ------- | ---------------- | --------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------ |
| `mov`   | `oxideav-mov`    | `mov` demuxer + muxer, `.mov` / `.qt` extensions, `ftyp qt  ` content probe | `audio`, `video`  | entry point lives at `oxideav_mov::registry::__oxideav_entry` (build.rs `ENTRY_PATH_OVERRIDES`)        |
| `riff`  | `oxideav-riff`   | nothing — RIFF chunk-walker + WAV/BWF metadata decoders, library-only       | `audio`, `video`  | no `register` fn in the crate; listed in `ENABLED_LIBRARY_ONLY_SIBLINGS`, absent from `ENABLED_SIBLINGS` |
| `tta`   | `oxideav-tta`    | `tta` codec (decode + encode), `tta` demuxer, `.tta` extension, `TTA1` probe | `audio`           |                                                                                                        |
| `svq`   | `oxideav-svq`    | `svq1` codec (decode + encode) + `svq3` decoder; FourCC tags `SVQ1` / `svqi` / `SVQ3` | `video` |                                                                                                        |

Pending first release — these siblings exist in the workspace but have
no live crates.io version, so meta cannot depend on them yet (consumer
CI resolves from the registry, not the umbrella patch table). They get
wired the round after their first release lands:

| Crate              | Status                                                  |
| ------------------ | ------------------------------------------------------- |
| `oxideav-dts`      | only published version is yanked — no resolvable release |
| `oxideav-indeo`    | unpublished                                             |
| `oxideav-lagarith` | unpublished                                             |

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

`ENABLED_LIBRARY_ONLY_SIBLINGS: &[(&str, &str)]` lists the siblings the
active feature set pulled into the dependency tree that expose **no**
register entry point (today: `oxideav-riff`). They are never dispatched
by `register_all`, never appear in `ENABLED_SIBLINGS`, and
`category_of` returns `None` for them.

Both slices are alphabetised by crate name (the same order
`register_all` calls them in) and short names are guaranteed to equal
the crate name with `oxideav-` stripped.

### Categorisation surface

`ENABLED_SIBLINGS_BY_CATEGORY: &[(&str, &[&str])]` groups every entry
in `ENABLED_SIBLINGS` under a stable category label so CLIs and
diagnostics can render an organised listing without a second pass.
The category set is fixed across builds and emitted in this order:
`audio-codec`, `video-codec`, `image-codec`, `audio-filter`,
`image-filter`, `container`, `subtitle`, `source`, `hwaccel`,
`delegation`, `render`. Empty categories are omitted, so a slim
feature set (e.g. `--features image`) yields a compact slice rather
than a sea of empty sections.

```ignore
for (category, shorts) in oxideav_meta::ENABLED_SIBLINGS_BY_CATEGORY {
    println!("{category}: {}", shorts.join(", "));
}
```

A companion `const fn category_of(short: &str) -> Option<&'static str>`
returns the same label for an individual short name (or `None` for
short names that `register_all` never dispatches — the 3D-format
crates route through `populate_mesh3d_registry`, so `category_of`
returns `None` for `mesh3d` / `stl` / `obj` / `gltf` / `usdz` /
`fbx`):

```ignore
assert_eq!(oxideav_meta::category_of("aac"), Some("audio-codec"));
assert_eq!(oxideav_meta::category_of("mp4"), Some("container"));
assert_eq!(oxideav_meta::category_of("mesh3d"), None);
```

Because `category_of` is `const fn`, callers can fold it into `const`
lookups and `static` initialisers. Both the slice and `category_of`
are emitted from the same `CATEGORY_TABLE` in `build.rs`, so they
stay structurally identical across releases; the integration suite
locks every direction of drift (sort order within a section, sections
are pairwise disjoint, every enabled sibling has exactly one entry,
round-trip parity between the slice and `category_of`).

## Resolution order

`register_all` calls siblings **alphabetically by crate name** (the
build script sorts the dependency walk), so "registration order" below
always means that order. What a collision resolves to depends on which
sub-registry the claim landed in:

| Surface                              | Rule (from `oxideav-core`)                                                                                         | On a tie                                                                        |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| codec FourCC / wFormatTag tags       | every claimant's probe is scored; highest confidence wins (no probe = `1.0`, `0.0` = withdraw)                     | first registration — the alphabetically-first crate                             |
| codec payload magics                 | longest matching prefix wins                                                                                       | first registration                                                              |
| container content probes             | highest `ProbeScore` wins                                                                                          | **unspecified** — the registry walks a `HashMap`; siblings must not tie         |
| container names / extensions         | last `register_demuxer` / `register_muxer` / `register_extension` wins                                             | the alphabetically-last crate silently shadows the earlier one                  |

Known overlaps, and how they are resolved today:

- `ftyp` files: both `oxideav-mov` (QTFF) and `oxideav-mp4` (ISO BMFF)
  probe the `ftyp` box. `oxideav-mov` returns `100` for a `qt  ` major
  brand, `90` for `qt  ` in the compatible-brand list, and `0` for any
  other `ftyp`, so a plain `isom` / `mp41` / … file always falls
  through to `mp4`. **Known tie:** `oxideav-mp4` scores a
  `qt  `-branded `ftyp` at the same top score, so with both features
  enabled a QuickTime-branded file opens as *either* `mov` or `mp4`
  depending on `HashMap` order (`KNOWN_PROBE_TIES` in
  `tests/sibling_wiring.rs`). Explicit `open_demuxer("mov", …)` and the
  `.mov` / `.qt` extension hints are unaffected — extensions are
  disjoint. The tie disappears once `oxideav-mp4` yields `qt  ` brands
  the way `oxideav-mov` yields non-QT ones, or once the core registry
  breaks probe ties by registration order.
- `svq1` / `svqi` / `SVQ3` FourCCs: claimed only by `oxideav-svq`.
- Codec-id aliases: the `all` build carries 15 multiply-claimed codec
  tags (`HEV1` / `HVC1` / `HEVC` / `V_MPEGH/ISO/HEVC` / MP4 object type
  35 under both `h265` and `hevc`; `DIVX` / `XVID` / `MP4V` / … under
  both `mpeg4video` and the hardware bridge's `mpeg4`; `WAVE_FORMAT`
  `0x0001` / `0x0003` under the PCM width variants, `0x0014` under
  `adpcm_g726` and `g723_1`, `0x0050` under `mp1` and `mp2`). All of
  them resolve by probe confidence with the alphabetically-first
  registrant winning ties — the pure-Rust `h265` / `mpeg4video` crates
  come before the hardware bridges — and the PCM / float variants
  resolve only once the container supplies `bits_per_sample`. The
  audit prints the full list (`--nocapture`).
- `TTA1`: claimed only by `oxideav-tta`'s container probe.
- `RIFF`: `oxideav-riff` registers nothing, so RIFF-family files are
  claimed by the format-specific siblings (AVI, WebP, …) as before.

`tests/sibling_wiring.rs` locks these: a codec-tag audit (every
multiply-claimed tag resolves to one of its claimants), a
payload-magic audit (identical magics claimed by two ids fail the
suite — there is no probe step to disambiguate them), and a
container-probe determinism check that builds a dozen fresh registries
and requires the same winner for each synthetic input (`ftyp qt  `,
`ftyp isom`, `TTA1`, a minimal RIFF/WAVE) — except for the ties
pinned in `KNOWN_PROBE_TIES`, whose winner only has to stay inside the
documented set. A flake there means two siblings tie on a content
probe and one of them must yield.

## How the build script works

The build script is a 200-line Rust program in `build.rs`:

1. Reads this crate's own `Cargo.toml`.
2. Walks the `[dependencies]` table and the recognized `[target.'cfg(...)'.dependencies]` tables (macOS / linux / linux-or-windows) and collects every `oxideav-*` dep.
3. For each dep, checks whether its corresponding `CARGO_FEATURE_*` env var is set (cargo sets these for every enabled feature). If yes, emits a `oxideav_<short>::__oxideav_entry(ctx)` call into the generated `register_all` — or the path from `ENTRY_PATH_OVERRIDES` for siblings that keep the macro-generated entry inside a submodule (`oxideav_mov::registry::__oxideav_entry`). Deps in `LIBRARY_ONLY` (`oxideav-riff`) get no call and land in `ENABLED_LIBRARY_ONLY_SIBLINGS` instead. When the dep was target-gated, the same `cfg(...)` body is emitted verbatim as a `#[cfg(...)]` attribute on the call so cross-target builds still produce working code.
4. Same logic for the five 3D-format crates → calls into `populate_mesh3d_registry`.
5. Emits two static-slice introspection constants — [`ENABLED_SIBLINGS`] (current-target view, with target gates evaluated against `CARGO_CFG_TARGET_OS`) and [`ENABLED_SIBLINGS_ALL`] (cross-target superset). When `mesh3d` is on, also emits `ENABLED_MESH3D_FORMATS`.
6. Writes the generated module to `$OUT_DIR/register_all.rs`; `src/lib.rs` pulls it in via `include!()`.

Adding a new sibling = add an optional dep line + a `name = ["dep:oxideav-name"]` feature line in `Cargo.toml`, plus a `CATEGORY_TABLE` row in `build.rs` (the build fails loud without it). The next build regenerates `register_all` automatically. Check where the sibling invokes `oxideav_core::register!` — if it is not at the crate root (or re-exported there), add an `ENTRY_PATH_OVERRIDES` row; if the crate has no `register` at all, add it to `LIBRARY_ONLY`.

## License

MIT.
