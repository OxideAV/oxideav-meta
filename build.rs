//! Auto-generates `register_all(ctx)` from `Cargo.toml` + active feature flags.
//!
//! Each sibling crate listed under `[dependencies]` or under one of
//! the recognized `[target.'cfg(...)'.dependencies]` sections gets a
//! `crate::register(ctx)` call in `register_all` **only if its
//! corresponding cargo feature is enabled** (`CARGO_FEATURE_<NAME>`
//! env var). Consumers depending on us with `default-features = false`
//! and a custom feature subset get a slimmer `register_all` body with
//! only the crates they asked for.
//!
//! Recognized target gates (the `cfg(...)` body is emitted verbatim as
//! a `#[cfg(...)]` attribute on the generated `register` call so the
//! cross-target build still produces working code):
//!
//! - `target_os = "macos"`           — VideoToolbox / AudioToolbox
//! - `target_os = "linux"`            — VA-API / VDPAU / NVIDIA
//! - `any(target_os = "linux", target_os = "windows")` — Vulkan Video
//!
//! Feature name = crate short name (with hyphens preserved), with one
//! special case: `oxideav-mod` maps to feature `amiga-mod` (avoids
//! shadowing the `mod` Rust keyword in feature lists).
//!
//! The generated file is emitted to `$OUT_DIR/register_all.rs` and
//! pulled into `src/lib.rs` via `include!()`. Adding a sibling = add an
//! optional dep + a `name = ["dep:oxideav-name"]` feature line in
//! Cargo.toml; this script regenerates `register_all` on the next build.
//!
//! No linkme, no DCE concerns, no `#[used]` magic, no wasm caveats —
//! explicit fn calls force the linker to keep each rlib alive
//! naturally.

use std::env;
use std::fs;
use std::path::PathBuf;

/// Crates we depend on but that should NOT be in `register_all`:
/// - `oxideav-core` is the foundation, not a register-emitting sibling.
/// - The 3D crates (`oxideav-mesh3d` + the four format siblings) use a
///   separate dispatch contract: they register into a
///   `oxideav_mesh3d::Mesh3DRegistry` via `populate_mesh3d_registry`,
///   not into the codec/container/filter/source RuntimeContext that
///   `register_all` walks.
const SKIP: &[&str] = &[
    "oxideav-core",
    "oxideav-mesh3d",
    "oxideav-stl",
    "oxideav-obj",
    "oxideav-gltf",
    "oxideav-usdz",
    "oxideav-fbx",
];

/// 3D format-codec crates. Each exposes `pub fn register(&mut
/// oxideav_mesh3d::Mesh3DRegistry)` (gated behind its default-on
/// `registry` feature). `populate_mesh3d_registry` calls each enabled
/// crate's `register` so a single helper builds a full registry.
const MESH3D_FORMAT_CRATES: &[&str] = &["stl", "obj", "gltf", "usdz", "fbx"];

/// Stable category labels emitted into `ENABLED_SIBLINGS_BY_CATEGORY`
/// and returned by `category_of(short)`. The order here defines the
/// section order of the by-category slice.
///
/// Section taxonomy mirrors the `Cargo.toml` organisational comments:
///
/// - `audio-codec` / `video-codec` / `image-codec` — pure-Rust codec
///   siblings, grouped by media kind.
/// - `audio-filter` / `image-filter` — DSP / pixel-filter siblings.
/// - `container` — mux/demux siblings (AVI, MP4, MKV, OGG, IFF, MOD,
///   S3M, FLV, MPEG-TS, …).
/// - `subtitle` — text + image subtitle siblings.
/// - `source` — URI source-driver siblings (`file://`, `http(s)://`,
///   `generate://`, `bluray://`, `dvd://`).
/// - `hwaccel` — OS-engine bridge siblings (VideoToolbox /
///   AudioToolbox / VA-API / VDPAU / NVIDIA / Vulkan Video).
/// - `delegation` — Windows-codec delegation bridge (`oxideav-vfw`).
/// - `render` — `Scene3D → RGBA8` renderer backends.
const CATEGORY_ORDER: &[&str] = &[
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

/// Static `(short_name, category)` mapping for every sibling whose
/// `__oxideav_entry` is dispatched by `register_all`. The 3D-format
/// crates + `oxideav-core` + `oxideav-mesh3d` are intentionally absent
/// — they're in `SKIP` and don't reach `register_all`. Entries here
/// must remain in sync with the `[dependencies]` section of
/// `Cargo.toml`; a missing short-name causes the
/// `category_of_known_short` build-time check below to fail loud so
/// future sibling-additions can't silently miss a category.
const CATEGORY_TABLE: &[(&str, &str)] = &[
    // Audio codecs.
    ("aac", "audio-codec"),
    ("ac3", "audio-codec"),
    ("ac4", "audio-codec"),
    ("adpcm", "audio-codec"),
    ("celt", "audio-codec"),
    ("flac", "audio-codec"),
    ("g711", "audio-codec"),
    ("g722", "audio-codec"),
    ("g7231", "audio-codec"),
    ("g728", "audio-codec"),
    ("g729", "audio-codec"),
    ("gsm", "audio-codec"),
    ("ilbc", "audio-codec"),
    ("mp1", "audio-codec"),
    ("mp2", "audio-codec"),
    ("mp3", "audio-codec"),
    ("opus", "audio-codec"),
    ("shorten", "audio-codec"),
    ("speex", "audio-codec"),
    ("vorbis", "audio-codec"),
    // Video codecs.
    ("amv", "video-codec"),
    ("av1", "video-codec"),
    ("cinepak", "video-codec"),
    ("dirac", "video-codec"),
    ("ffv1", "video-codec"),
    ("h261", "video-codec"),
    ("h263", "video-codec"),
    ("h264", "video-codec"),
    ("h265", "video-codec"),
    ("h266", "video-codec"),
    ("huffyuv", "video-codec"),
    ("magicyuv", "video-codec"),
    ("mjpeg", "video-codec"),
    ("mpeg12video", "video-codec"),
    ("mpeg4video", "video-codec"),
    ("msmpeg4", "video-codec"),
    ("prores", "video-codec"),
    ("theora", "video-codec"),
    ("utvideo", "video-codec"),
    ("vp6", "video-codec"),
    ("vp8", "video-codec"),
    ("vp9", "video-codec"),
    // Image codecs.
    ("avif", "image-codec"),
    ("dds", "image-codec"),
    ("gif", "image-codec"),
    ("icer", "image-codec"),
    ("jpeg2000", "image-codec"),
    ("jpegxl", "image-codec"),
    ("jpegxs", "image-codec"),
    ("openexr", "image-codec"),
    ("pbm", "image-codec"),
    ("pdf", "image-codec"),
    ("pict", "image-codec"),
    ("png", "image-codec"),
    ("qoi", "image-codec"),
    ("svg", "image-codec"),
    ("webp", "image-codec"),
    // Filters.
    ("audio-filter", "audio-filter"),
    ("image-filter", "image-filter"),
    // Containers.
    ("avi", "container"),
    ("basic", "container"),
    ("flv", "container"),
    ("iff", "container"),
    ("mkv", "container"),
    ("mod", "container"),
    ("mp4", "container"),
    ("ogg", "container"),
    ("s3m", "container"),
    ("mpegts", "container"),
    // Subtitles.
    ("ass", "subtitle"),
    ("sub-image", "subtitle"),
    ("subtitle", "subtitle"),
    // Source drivers.
    ("source", "source"),
    ("http", "source"),
    ("generator", "source"),
    ("bluray", "source"),
    ("dvd", "source"),
    // Hardware-accel bridges.
    ("audiotoolbox", "hwaccel"),
    ("videotoolbox", "hwaccel"),
    ("vaapi", "hwaccel"),
    ("vdpau", "hwaccel"),
    ("nvidia", "hwaccel"),
    ("vulkan-video", "hwaccel"),
    // Delegation bridge.
    ("vfw", "delegation"),
    // 3D renderer.
    ("render", "render"),
];

/// Lookup helper used at build time: panics if `short` isn't in
/// `CATEGORY_TABLE`. Called for every sibling the build script is about
/// to emit so a new dep added to `Cargo.toml` without a corresponding
/// `CATEGORY_TABLE` entry fails the build instead of silently
/// disappearing from `ENABLED_SIBLINGS_BY_CATEGORY`.
fn category_of_known_short(short: &str) -> &'static str {
    for (s, cat) in CATEGORY_TABLE {
        if *s == short {
            return cat;
        }
    }
    panic!(
        "CATEGORY_TABLE in build.rs missing entry for sibling short name {short:?} — \
         add a (\"{short}\", \"<category>\") row to keep ENABLED_SIBLINGS_BY_CATEGORY complete"
    );
}

/// Each section's parser key (the normalized `Cargo.toml` table
/// header with whitespace stripped) paired with the `cfg(...)` body
/// to emit on the generated `register` call. `None` body = the plain
/// `[dependencies]` section, no cfg attribute.
const SECTIONS: &[(&str, Option<&str>)] = &[
    ("dependencies", None),
    (
        "target.'cfg(target_os=\"macos\")'.dependencies",
        Some("target_os = \"macos\""),
    ),
    (
        "target.'cfg(target_os=\"linux\")'.dependencies",
        Some("target_os = \"linux\""),
    ),
    (
        "target.'cfg(any(target_os=\"linux\",target_os=\"windows\"))'.dependencies",
        Some("any(target_os = \"linux\", target_os = \"windows\")"),
    ),
];

/// Special feature-name overrides for crates whose feature name doesn't
/// equal the crate short name. Currently just `oxideav-mod` →
/// `amiga-mod` (avoids the `mod` Rust keyword as a feature).
fn feature_name_for(crate_short: &str) -> String {
    match crate_short {
        "mod" => "amiga-mod".to_string(),
        other => other.to_string(),
    }
}

/// `CARGO_FEATURE_X` env var name for a feature. Cargo upcases the
/// feature name and replaces dashes with underscores.
fn env_var_for_feature(feat: &str) -> String {
    format!("CARGO_FEATURE_{}", feat.replace('-', "_").to_uppercase())
}

/// Evaluate a `SECTIONS` gate body (e.g. `target_os = "macos"` or
/// `any(target_os = "linux", target_os = "windows")`) against the
/// current build's target OS.
///
/// Only the gate shapes used by `SECTIONS` are recognised — anything
/// else returns `false` to err on the safe side (the gated entry stays
/// out of [`ENABLED_SIBLINGS`]). The corresponding `#[cfg(...)]` on the
/// `register_all` call is the authoritative check at compile time; the
/// build-script evaluator only feeds the introspection slice.
fn gate_matches_target(gate: &str, target_os: &str) -> bool {
    let g: String = gate.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(rest) = g.strip_prefix("target_os=\"") {
        if let Some(os) = rest.strip_suffix('"') {
            return os == target_os;
        }
    }
    if let Some(inner) = g.strip_prefix("any(").and_then(|s| s.strip_suffix(')')) {
        // any(target_os="linux",target_os="windows")
        return inner
            .split(',')
            .any(|term| gate_matches_target(term, target_os));
    }
    false
}

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");

    let manifest = fs::read_to_string("Cargo.toml").expect("read Cargo.toml");
    let deps = collect_sibling_deps(&manifest);

    let mut out = String::new();
    out.push_str(
        "// Auto-generated by build.rs from Cargo.toml + active feature flags. Do not edit.\n",
    );
    out.push_str("//\n");
    out.push_str("// `register_all(ctx)` explicitly invokes every enabled sibling's\n");
    out.push_str("// `__oxideav_entry(ctx)` wrapper (generated by the\n");
    out.push_str("// `oxideav_core::register!` macro at the sibling's call site).\n");
    out.push_str("// Each call forces the linker to keep the sibling rlib alive\n");
    out.push_str("// (no DCE concerns).\n\n");
    out.push_str("/// Walk every sibling crate enabled at build time and invoke its\n");
    out.push_str("/// `__oxideav_entry(ctx)` wrapper. The call set is determined by which cargo\n");
    out.push_str("/// features are active when this crate is built; see Cargo.toml\n");
    out.push_str("/// for the audio/video/image/subtitle/hwaccel preset bundles +\n");
    out.push_str("/// per-crate features.\n");
    out.push_str("pub fn register_all(ctx: &mut oxideav_core::RuntimeContext) {\n");
    // Silence `unused_variables` when no register-emitting feature is
    // enabled (e.g. `--features 3d` builds where every dep is in SKIP).
    out.push_str("    let _ = ctx;\n");

    // Evaluate target gates against the current cargo target so the
    // ENABLED_SIBLINGS slice below can omit entries whose target gate
    // doesn't match. Build.rs runs per-target, so `CARGO_CFG_TARGET_OS`
    // names the OS the compiled artefact will run on.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Collect (crate_name, short_name, gate) for every sibling whose
    // feature is enabled — used both for the register_all body and the
    // ENABLED_SIBLINGS_ALL static slice below.
    let mut enabled: Vec<(String, String, Option<String>)> = Vec::new();
    for (full_name, gate) in &deps {
        let short = full_name.strip_prefix("oxideav-").unwrap_or(full_name);
        let feature = feature_name_for(short);
        let env_var = env_var_for_feature(&feature);
        if env::var_os(&env_var).is_none() {
            continue; // feature off, skip
        }
        let krate = format!("oxideav_{}", short.replace('-', "_"));
        if let Some(g) = gate {
            out.push_str(&format!("    #[cfg({g})]\n"));
        }
        out.push_str(&format!("    {krate}::__oxideav_entry(ctx);\n"));
        enabled.push((full_name.clone(), short.to_string(), gate.clone()));
    }

    out.push_str("}\n");

    // Static-slice introspection: a `(crate_name, short_name)` entry
    // for every sibling whose `__oxideav_entry` is called by
    // `register_all` on the *current target*. Useful for diagnostics
    // ("did meta wire codec X at build time?"), CLI listings, and
    // integration tests.
    //
    // Build.rs evaluates each entry's target gate against
    // `CARGO_CFG_TARGET_OS`: gated entries (macOS / linux / windows
    // HW-accel crates) appear in the slice only on a matching target.
    // `ENABLED_SIBLINGS_ALL` below is a target-independent superset.
    out.push('\n');
    out.push_str("/// Crate-name + short-name pairs for every sibling whose\n");
    out.push_str("/// `__oxideav_entry` is dispatched by [`register_all`] on the current\n");
    out.push_str("/// build target. Compile-time constant; entries are filtered by the\n");
    out.push_str("/// active feature flags and the target's OS gate (HW-accel crates).\n");
    out.push_str("///\n");
    out.push_str("/// Example:\n");
    out.push_str("///\n");
    out.push_str("/// ```ignore\n");
    out.push_str("/// for (crate_name, short) in oxideav_meta::ENABLED_SIBLINGS {\n");
    out.push_str(
        "///     println!(\"meta wires {crate_name} (short = {short}) into register_all\");\n",
    );
    out.push_str("/// }\n");
    out.push_str("/// ```\n");
    out.push_str("pub const ENABLED_SIBLINGS: &[(&str, &str)] = &[\n");
    for (full_name, short, gate) in &enabled {
        if let Some(g) = gate {
            if !gate_matches_target(g, &target_os) {
                continue;
            }
        }
        out.push_str(&format!("    (\"{full_name}\", \"{short}\"),\n"));
    }
    out.push_str("];\n");

    // Cross-target companion: every sibling that would be wired if its
    // target gate were satisfied. Strict superset of `ENABLED_SIBLINGS`.
    // Useful for tooling that wants to enumerate "every backend meta
    // knows about" regardless of the current build target.
    out.push('\n');
    out.push_str("/// Like [`ENABLED_SIBLINGS`] but also includes target-gated entries\n");
    out.push_str("/// (macOS / linux / windows HW-accel crates) whose `#[cfg(...)]` gate\n");
    out.push_str("/// does not let them link on the current target. Strict superset of\n");
    out.push_str("/// [`ENABLED_SIBLINGS`]. Order: alphabetical by crate name.\n");
    out.push_str("pub const ENABLED_SIBLINGS_ALL: &[(&str, &str)] = &[\n");
    for (full_name, short, _gate) in &enabled {
        out.push_str(&format!("    (\"{full_name}\", \"{short}\"),\n"));
    }
    out.push_str("];\n");

    // ENABLED_SIBLINGS_BY_CATEGORY: a `(category, &[short_name])` slice
    // grouping every entry in `ENABLED_SIBLINGS` under a stable category
    // label. Categories are emitted in the fixed `CATEGORY_ORDER` so
    // tooling can rely on a deterministic iteration order; within a
    // category, short names inherit the alphabetical sort already
    // imposed by `collect_sibling_deps`. Empty categories are omitted
    // so a slim feature subset (e.g. `--features image`) produces a
    // compact slice instead of a list of empty sections.
    //
    // Source-of-truth is `CATEGORY_TABLE` above — the build fails loud
    // (via `category_of_known_short`) if any enabled sibling is missing
    // a category row, so the slice and `category_of()` stay in sync
    // with the dep list.
    out.push('\n');
    out.push_str("/// Crate-short-name lists for every category the active feature set\n");
    out.push_str("/// exposes. Companion to [`ENABLED_SIBLINGS`]: same target-gate\n");
    out.push_str("/// filtering, same alphabetical sort within a category, but grouped\n");
    out.push_str("/// by [`category_of`] so CLIs and diagnostics can render an organised\n");
    out.push_str("/// listing without a second pass.\n");
    out.push_str("///\n");
    out.push_str("/// Category order is stable across builds — `audio-codec` first, then\n");
    out.push_str("/// `video-codec`, `image-codec`, `audio-filter`, `image-filter`,\n");
    out.push_str("/// `container`, `subtitle`, `source`, `hwaccel`, `delegation`,\n");
    out.push_str("/// `render`. Empty categories are omitted so a slim build (e.g.\n");
    out.push_str("/// `default-features = false, features = [\"image\"]`) emits a\n");
    out.push_str("/// compact slice rather than a sea of empty sections.\n");
    out.push_str("///\n");
    out.push_str("/// Every short name appears in exactly one category, and the union\n");
    out.push_str("/// across categories equals the short-name column of\n");
    out.push_str("/// [`ENABLED_SIBLINGS`]; the smoke-test suite locks both properties.\n");
    out.push_str("pub const ENABLED_SIBLINGS_BY_CATEGORY: &[(&str, &[&str])] = &[\n");
    // Build a per-category vector of (filtered) short names. Filtering
    // matches the ENABLED_SIBLINGS rule: skip a sibling whose target
    // gate doesn't satisfy the current build target.
    for category in CATEGORY_ORDER {
        let mut shorts_in_cat: Vec<&str> = Vec::new();
        for (_full_name, short, gate) in &enabled {
            if let Some(g) = gate {
                if !gate_matches_target(g, &target_os) {
                    continue;
                }
            }
            if category_of_known_short(short) == *category {
                shorts_in_cat.push(short.as_str());
            }
        }
        if shorts_in_cat.is_empty() {
            continue;
        }
        out.push_str(&format!("    (\"{category}\", &[\n"));
        for s in &shorts_in_cat {
            out.push_str(&format!("        \"{s}\",\n"));
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");

    // `category_of(short)` — `const fn` lookup from short name to
    // category label. Returns `None` for short names that aren't wired
    // by `register_all` (3D format crates in `SKIP`, unknown strings,
    // …). The match arm list is exactly `CATEGORY_TABLE`, emitted in
    // declaration order — `category_of` doesn't depend on which
    // features are active, so a downstream caller can ask "what
    // category WOULD `aac` belong to?" without first enabling its
    // feature.
    out.push('\n');
    out.push_str("/// Look up the stable category label for a sibling short name. Returns\n");
    out.push_str("/// `None` for short names that `register_all` never dispatches\n");
    out.push_str("/// (`oxideav-mesh3d`, `oxideav-stl`/`obj`/`gltf`/`usdz`/`fbx` — these\n");
    out.push_str("/// route through `populate_mesh3d_registry` instead — and any string\n");
    out.push_str("/// that isn't a known oxideav sibling).\n");
    out.push_str("///\n");
    out.push_str("/// `const fn` so callers can fold it into `const` lookups and `static`\n");
    out.push_str("/// initialisers. The category set is the same as the one\n");
    out.push_str("/// [`ENABLED_SIBLINGS_BY_CATEGORY`] emits headers for — feature-state\n");
    out.push_str("/// independent (every known short name resolves regardless of which\n");
    out.push_str("/// cargo features are active in the current build).\n");
    out.push_str("///\n");
    out.push_str("/// ```ignore\n");
    out.push_str("/// assert_eq!(oxideav_meta::category_of(\"aac\"), Some(\"audio-codec\"));\n");
    out.push_str("/// assert_eq!(oxideav_meta::category_of(\"mp4\"), Some(\"container\"));\n");
    out.push_str("/// assert_eq!(oxideav_meta::category_of(\"mesh3d\"), None);\n");
    out.push_str("/// ```\n");
    out.push_str("pub const fn category_of(short: &str) -> Option<&'static str> {\n");
    out.push_str("    // const fn => no closures, no Iterator combinators; expand the\n");
    out.push_str("    // CATEGORY_TABLE into a flat byte-slice match.\n");
    out.push_str("    let b = short.as_bytes();\n");
    out.push_str("    let mut i = 0;\n");
    out.push_str("    let table: &[(&str, &str)] = &[\n");
    for (s, cat) in CATEGORY_TABLE {
        out.push_str(&format!("        (\"{s}\", \"{cat}\"),\n"));
    }
    out.push_str("    ];\n");
    out.push_str("    while i < table.len() {\n");
    out.push_str("        let key = table[i].0.as_bytes();\n");
    out.push_str("        if bytes_eq(key, b) {\n");
    out.push_str("            return Some(table[i].1);\n");
    out.push_str("        }\n");
    out.push_str("        i += 1;\n");
    out.push_str("    }\n");
    out.push_str("    None\n");
    out.push_str("}\n");
    out.push('\n');
    out.push_str("// Local helper for `category_of`: const-friendly slice equality\n");
    out.push_str("// (stable Rust's `PartialEq` for `&[u8]` isn't `const`).\n");
    out.push_str("const fn bytes_eq(a: &[u8], b: &[u8]) -> bool {\n");
    out.push_str("    if a.len() != b.len() {\n");
    out.push_str("        return false;\n");
    out.push_str("    }\n");
    out.push_str("    let mut i = 0;\n");
    out.push_str("    while i < a.len() {\n");
    out.push_str("        if a[i] != b[i] {\n");
    out.push_str("            return false;\n");
    out.push_str("        }\n");
    out.push_str("        i += 1;\n");
    out.push_str("    }\n");
    out.push_str("    true\n");
    out.push_str("}\n");

    // Second generated function: `populate_mesh3d_registry`. Lives
    // behind `#[cfg(feature = "mesh3d")]` so `oxideav_mesh3d` is in
    // scope. Each enabled 3D-format crate's `register(&mut
    // Mesh3DRegistry)` is called in turn (gated on its own feature).
    out.push('\n');
    out.push_str("/// Populate a [`oxideav_mesh3d::Mesh3DRegistry`] with every enabled\n");
    out.push_str("/// 3D-format codec sibling's decoder + encoder factories.\n");
    out.push_str("///\n");
    out.push_str("/// Parallel to [`register_all`] but for the separate Mesh3DRegistry\n");
    out.push_str("/// dispatch contract that 3D-format crates use. Only available when\n");
    out.push_str("/// the `mesh3d` cargo feature is enabled (i.e. when `oxideav-mesh3d`\n");
    out.push_str("/// is in the dep tree).\n");
    out.push_str("#[cfg(feature = \"mesh3d\")]\n");
    out.push_str(
        "pub fn populate_mesh3d_registry(registry: &mut oxideav_mesh3d::Mesh3DRegistry) {\n",
    );
    // Silence `unused_variables` when only `mesh3d` is on (no format
    // codecs enabled) — registry stays empty but compiles cleanly.
    out.push_str("    let _ = registry;\n");
    let mut enabled_mesh: Vec<&'static str> = Vec::new();
    for short in MESH3D_FORMAT_CRATES {
        let env_var = env_var_for_feature(short);
        if env::var_os(&env_var).is_none() {
            continue;
        }
        let krate = format!("oxideav_{}", short.replace('-', "_"));
        out.push_str(&format!("    {krate}::register(registry);\n"));
        enabled_mesh.push(short);
    }
    out.push_str("}\n");

    // Static-slice introspection counterpart for 3D-format crates.
    // Gated on `#[cfg(feature = "mesh3d")]` (matching the helper fn).
    out.push('\n');
    out.push_str("/// Short names of every 3D-format sibling whose `register` is called by\n");
    out.push_str("/// [`populate_mesh3d_registry`]. Only present when the `mesh3d`\n");
    out.push_str("/// feature is enabled.\n");
    out.push_str("#[cfg(feature = \"mesh3d\")]\n");
    out.push_str("pub const ENABLED_MESH3D_FORMATS: &[&str] = &[\n");
    for short in &enabled_mesh {
        out.push_str(&format!("    \"{short}\",\n"));
    }
    out.push_str("];\n");

    // Third generated function: `populate_render_registry`. Parallel to
    // `populate_mesh3d_registry` but for the `oxideav_render::RenderRegistry`
    // dispatch contract. Today populates `"scanline"` via
    // `oxideav_render::register_into`; future render-backend siblings
    // (raycaster, path-tracer) will be added here. Gated on
    // `#[cfg(feature = "render")]` so slim builds without 3D in the dep
    // tree compile cleanly.
    out.push('\n');
    out.push_str("/// Populate a [`oxideav_render::RenderRegistry`] with every render\n");
    out.push_str("/// backend the active feature set exposes.\n");
    out.push_str("///\n");
    out.push_str("/// Today that's just `oxideav_render::register_into`, which registers\n");
    out.push_str("/// `\"scanline\"`; future render-backend siblings (raycaster,\n");
    out.push_str("/// path-tracer) will be wired in here. Parallel to [`register_all`]\n");
    out.push_str("/// and [`populate_mesh3d_registry`] but for the separate\n");
    out.push_str("/// `RenderRegistry` dispatch contract. Only available when the\n");
    out.push_str("/// `render` cargo feature is enabled (i.e. when `oxideav-render` is\n");
    out.push_str("/// in the dep tree; the `3d` preset bundle pulls it in).\n");
    out.push_str("#[cfg(feature = \"render\")]\n");
    out.push_str("pub fn populate_render_registry(reg: &mut oxideav_render::RenderRegistry) {\n");
    out.push_str("    oxideav_render::register_into(reg);\n");
    out.push_str("}\n");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let dest = out_dir.join("register_all.rs");
    fs::write(&dest, out).expect("write register_all.rs");
}

/// Parse `Cargo.toml` for `oxideav-*` deps in any of the recognized
/// `SECTIONS` headers. Returns `Vec<(crate_name, cfg_gate)>` where
/// `cfg_gate = None` means the dep was in `[dependencies]` (no
/// `#[cfg]` attribute on the generated call) and `Some(s)` means
/// emit `#[cfg(s)]` on the call.
///
/// Output is sorted: `None` gates first (so the `[dependencies]`
/// crates emit before the target-gated ones), then by gate string,
/// then by crate name. Deterministic across runs.
fn collect_sibling_deps(manifest: &str) -> Vec<(String, Option<String>)> {
    let mut deps: Vec<(String, Option<String>)> = Vec::new();
    let mut active: Option<Option<&'static str>> = None;

    for line in manifest.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            let header: String = trimmed
                .trim_start_matches('[')
                .trim_end_matches(']')
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            active = SECTIONS
                .iter()
                .find(|(key, _)| *key == header)
                .map(|(_, gate)| *gate);
            continue;
        }

        let Some(gate) = active else {
            continue;
        };
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let key = trimmed[..eq].trim();
        if !key.starts_with("oxideav-") || SKIP.contains(&key) {
            continue;
        }
        deps.push((key.to_string(), gate.map(|s| s.to_string())));
    }

    deps.sort();
    deps
}
