//! Locks the build script's manifest walker (`build/manifest.rs`,
//! included here verbatim) against BOTH manifest shapes:
//!
//! * the in-tree `Cargo.toml` (inline `name = { … }` entries under a
//!   section header), and
//! * the NORMALIZED manifest cargo writes into the published `.crate`
//!   (one `[dependencies.name]` table per dependency, comments gone).
//!
//! `oxideav-meta` 0.0.2 only understood the first, so every crates.io
//! consumer got an empty `register_all`. These tests fail if the walker
//! ever regresses to one shape; the `off-umbrella` CI job proves the
//! same thing end to end from a packaged `.crate`.
//!
//! No sibling features are needed — the harness only exercises the
//! parser, so it runs under `--no-default-features` too.

#[allow(dead_code)]
#[path = "../build/manifest.rs"]
mod manifest;

use manifest::{ManifestDeps, SKIP, TARGET_GATES, walk_manifest};

const IN_TREE_FIXTURE: &str = include_str!("fixtures/in-tree.toml");
const NORMALIZED_FIXTURE: &str = include_str!("fixtures/normalized.toml");
const LIVE_MANIFEST: &str = include_str!("../Cargo.toml");

const MACOS: &str = "target_os = \"macos\"";
const LINUX: &str = "target_os = \"linux\"";
const LINUX_OR_WINDOWS: &str = "any(target_os = \"linux\", target_os = \"windows\")";

fn deps(manifest: &str) -> Vec<(String, Option<String>)> {
    walk_manifest(manifest).deps
}

fn dep(name: &str, gate: Option<&str>) -> (String, Option<String>) {
    (name.to_string(), gate.map(str::to_string))
}

/// The dependency list both trimmed fixtures must produce.
fn fixture_expectation() -> Vec<(String, Option<String>)> {
    vec![
        dep("oxideav-aac", None),
        dep("oxideav-audiotoolbox", Some(MACOS)),
        dep("oxideav-mod", None),
        dep("oxideav-nvidia", Some(LINUX)),
        dep("oxideav-png", None),
        dep("oxideav-riff", None),
        dep("oxideav-vaapi", Some(LINUX)),
        dep("oxideav-vdpau", Some(LINUX)),
        dep("oxideav-videotoolbox", Some(MACOS)),
        dep("oxideav-vulkan-video", Some(LINUX_OR_WINDOWS)),
    ]
}

#[test]
fn in_tree_fixture_parses() {
    let walked = walk_manifest(IN_TREE_FIXTURE);
    assert_eq!(walked.deps, fixture_expectation());
    assert_eq!(
        walked.skipped,
        ["oxideav-core", "oxideav-mesh3d", "oxideav-stl"],
        "SKIP entries are reported, never emitted"
    );
    assert!(walked.unrecognized.is_empty());
}

#[test]
fn normalized_fixture_parses() {
    let walked = walk_manifest(NORMALIZED_FIXTURE);
    assert_eq!(walked.deps, fixture_expectation());
    assert_eq!(
        walked.skipped,
        ["oxideav-core", "oxideav-mesh3d", "oxideav-stl"]
    );
    assert!(walked.unrecognized.is_empty());
}

#[test]
fn both_shapes_yield_the_identical_dep_list() {
    assert_eq!(
        walk_manifest(IN_TREE_FIXTURE),
        walk_manifest(NORMALIZED_FIXTURE),
        "the in-tree and the cargo-normalized manifest must be indistinguishable to build.rs"
    );
}

#[test]
fn published_0_0_2_shape_is_not_empty() {
    // The exact failure of the published 0.0.2: a normalized manifest
    // with nothing but per-dependency tables parsed to zero deps.
    let normalized_only: String = NORMALIZED_FIXTURE
        .lines()
        .skip_while(|l| !l.starts_with("[dependencies."))
        .map(|l| format!("{l}\n"))
        .collect();
    assert!(normalized_only.starts_with("[dependencies.oxideav-aac]"));
    let walked = walk_manifest(&normalized_only);
    assert_eq!(walked.deps, fixture_expectation());
    assert!(walked.skipped.contains(&"oxideav-core".to_string()));
}

// ───────────────────────── live manifest ─────────────────────────

#[test]
fn live_manifest_parses_every_sibling() {
    let walked = walk_manifest(LIVE_MANIFEST);
    assert!(
        walked.deps.len() >= 100,
        "expected 100+ siblings in Cargo.toml, got {}",
        walked.deps.len()
    );
    assert!(walked.deps.contains(&dep("oxideav-png", None)));
    assert!(walked.deps.contains(&dep("oxideav-riff", None)));
    assert!(
        walked
            .deps
            .contains(&dep("oxideav-videotoolbox", Some(MACOS)))
    );
    assert!(walked.deps.contains(&dep("oxideav-vaapi", Some(LINUX))));
    assert!(
        walked
            .deps
            .contains(&dep("oxideav-vulkan-video", Some(LINUX_OR_WINDOWS)))
    );
    for skip in SKIP {
        assert!(
            !walked.deps.iter().any(|(n, _)| n == skip),
            "{skip} is in SKIP and must not be emitted"
        );
    }
    assert!(walked.skipped.contains(&"oxideav-core".to_string()));
    assert!(
        walked.unrecognized.is_empty(),
        "every target header in Cargo.toml must be in TARGET_GATES: {:?}",
        walked.unrecognized
    );
    // Sorted + unique, the property ENABLED_SIBLINGS relies on.
    let mut sorted = walked.deps.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(walked.deps, sorted);
}

/// Rewrite an in-tree manifest the way `cargo package` does for the
/// dependency sections: every inline `name = { … }` entry becomes its
/// own `[section.name]` table, comments and blank lines vanish, and
/// the tables come out in a different (here: reversed) order. Tables
/// that already have the normalized shape pass through unchanged, so
/// the function is idempotent — the test holds even when this harness
/// runs from inside a packaged `.crate`, where `../Cargo.toml` is
/// already the normalized manifest.
fn normalize_like_cargo(manifest: &str) -> String {
    let mut tables: Vec<String> = Vec::new();
    // `Some(header)` while inside a section whose `key = …` lines are
    // dependencies; `None` elsewhere (including inside a per-dependency
    // table, whose lines are appended to the last passthrough table).
    let mut section: Option<String> = None;
    let mut passthrough = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            let header = line.trim_start_matches('[').trim_end_matches(']');
            let is_section = header == "dependencies" || header.ends_with(".dependencies");
            let is_table = header.starts_with("dependencies.") || header.contains(".dependencies.");
            section = is_section.then(|| header.to_string());
            passthrough = is_table;
            if is_table {
                tables.push(format!("[{header}]\n"));
            }
            continue;
        }
        if passthrough {
            let last = tables.last_mut().expect("passthrough table header pushed");
            last.push_str(line);
            last.push('\n');
            continue;
        }
        let Some(sec) = &section else { continue };
        let Some(eq) = line.find('=') else { continue };
        let name = line[..eq].trim();
        let value = line[eq + 1..].trim();
        let body = if let Some(inner) = value.strip_prefix('{') {
            inner
                .trim_end_matches('}')
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            format!("version = {value}")
        };
        tables.push(format!("[{sec}.{name}]\n{body}\n"));
    }
    tables.reverse();
    tables.join("\n")
}

#[test]
fn live_manifest_normalized_round_trip() {
    let normalized = normalize_like_cargo(LIVE_MANIFEST);
    assert!(
        normalized.contains("[dependencies.oxideav-png]\nversion = \"0\"\noptional = true\n"),
        "synthesised normalization did not produce per-dependency tables:\n{normalized}"
    );
    assert!(
        normalized
            .contains("[target.'cfg(target_os = \"macos\")'.dependencies.oxideav-videotoolbox]\n")
    );
    assert_eq!(
        walk_manifest(&normalized),
        walk_manifest(LIVE_MANIFEST),
        "the live Cargo.toml and its cargo-normalized twin must parse identically"
    );
    // Idempotent on an already-normalized manifest (what `../Cargo.toml`
    // is when this harness runs from a packaged `.crate`).
    assert_eq!(
        walk_manifest(&normalize_like_cargo(NORMALIZED_FIXTURE)),
        walk_manifest(NORMALIZED_FIXTURE)
    );
    assert_eq!(
        walk_manifest(&normalize_like_cargo(&normalized)),
        walk_manifest(LIVE_MANIFEST)
    );
}

// ───────────────────── header / key edge cases ─────────────────────

#[test]
fn double_quoted_target_headers_match_the_same_gate() {
    let single = "[target.'cfg(target_os = \"macos\")'.dependencies.oxideav-videotoolbox]\nversion = \"0\"\n";
    let double = "[target.\"cfg(target_os = \\\"macos\\\")\".dependencies.oxideav-videotoolbox]\nversion = \"0\"\n";
    let squeezed = "[target.'cfg(target_os=\"macos\")'.dependencies.oxideav-videotoolbox]\n";
    let any_double = "[target.\"cfg(any(target_os = \\\"linux\\\", target_os = \\\"windows\\\"))\".dependencies]\noxideav-vulkan-video = { version = \"0\", optional = true }\n";
    assert_eq!(deps(single), [dep("oxideav-videotoolbox", Some(MACOS))]);
    assert_eq!(deps(double), deps(single));
    assert_eq!(deps(squeezed), deps(single));
    assert_eq!(
        deps(any_double),
        [dep("oxideav-vulkan-video", Some(LINUX_OR_WINDOWS))]
    );
}

#[test]
fn quoted_and_dotted_keys_name_the_dependency() {
    let m = "[dependencies]\n\"oxideav-png\" = { version = \"0\", optional = true }\noxideav-qoi.version = \"0\"\noxideav-qoi.optional = true\n[dependencies.\"oxideav-gif\"]\nversion = \"0\"\n";
    assert_eq!(
        deps(m),
        [
            dep("oxideav-gif", None),
            dep("oxideav-png", None),
            dep("oxideav-qoi", None)
        ]
    );
}

#[test]
fn whitespace_and_trailing_comments_on_headers() {
    let m = "[ dependencies ]  # the plain section\noxideav-png = \"0\" # trailing\n[ target . 'cfg(target_os = \"linux\")' . dependencies . oxideav-vaapi ] # gated\nversion = \"0\"\n";
    assert_eq!(
        deps(m),
        [dep("oxideav-png", None), dep("oxideav-vaapi", Some(LINUX))]
    );
}

#[test]
fn other_tables_never_contribute() {
    let m = "[package]\nname = \"oxideav-meta\"\n[dev-dependencies]\noxideav-aac = \"0\"\n[dev-dependencies.oxideav-png]\nversion = \"0\"\n[build-dependencies]\noxideav-qoi = \"0\"\n[features]\npng = [\"dep:oxideav-png\"]\n[[test]]\nname = \"oxideav-fake\"\n[lib]\nname = \"oxideav_meta\"\n[target.'cfg(target_os = \"macos\")'.dev-dependencies]\noxideav-videotoolbox = \"0\"\n[dependencies.oxideav-png]\nversion = \"0\"\nfoo-oxideav-bar = \"1\"\n";
    assert_eq!(deps(m), [dep("oxideav-png", None)]);
}

#[test]
fn table_bodies_are_not_mistaken_for_dependencies() {
    // `version = …` / `optional = …` lines inside a `[dependencies.x]`
    // table must not be read as deps named `version` / `optional`, and
    // a non-oxideav table must not leak either.
    let m = "[dependencies.oxideav-png]\nversion = \"0\"\noptional = true\n[dependencies.serde]\nversion = \"1\"\n[dependencies]\nlog = \"0.4\"\n";
    let walked = walk_manifest(m);
    assert_eq!(walked.deps, [dep("oxideav-png", None)]);
    assert!(walked.skipped.is_empty());
    assert!(walked.unrecognized.is_empty());
}

#[test]
fn skip_entries_are_reported_not_emitted() {
    let m = "[dependencies]\noxideav-core = \"0.1\"\noxideav-mesh3d = { version = \"0.0\", optional = true }\n[dependencies.oxideav-stl]\nversion = \"0.0\"\noptional = true\n[dependencies.oxideav-png]\nversion = \"0\"\n";
    let walked = walk_manifest(m);
    assert_eq!(walked.deps, [dep("oxideav-png", None)]);
    assert_eq!(
        walked.skipped,
        ["oxideav-core", "oxideav-mesh3d", "oxideav-stl"]
    );
}

#[test]
fn unknown_target_gates_are_reported_not_dropped() {
    let m = "[target.'cfg(target_os = \"freebsd\")'.dependencies]\noxideav-kqueue = \"0\"\nlibc = \"0.2\"\n[target.x86_64-unknown-linux-gnu.dependencies.oxideav-sse]\nversion = \"0\"\n[target.'cfg(target_os = \"linux\")'.dependencies.oxideav-vaapi]\nversion = \"0\"\n";
    let walked = walk_manifest(m);
    assert_eq!(walked.deps, [dep("oxideav-vaapi", Some(LINUX))]);
    assert_eq!(
        walked.unrecognized,
        [
            (
                "target.'cfg(target_os = \"freebsd\")'.dependencies".to_string(),
                "oxideav-kqueue".to_string()
            ),
            (
                "target.x86_64-unknown-linux-gnu.dependencies.oxideav-sse".to_string(),
                "oxideav-sse".to_string()
            ),
        ]
    );
}

#[test]
fn output_is_sorted_and_deduplicated() {
    let m = "[dependencies.oxideav-qoi]\nversion = \"0\"\n[dependencies]\noxideav-png = \"0\"\noxideav-aac = \"0\"\n[dependencies.oxideav-png]\nversion = \"0\"\n";
    assert_eq!(
        deps(m),
        [
            dep("oxideav-aac", None),
            dep("oxideav-png", None),
            dep("oxideav-qoi", None)
        ]
    );
}

#[test]
fn empty_or_unparseable_input_yields_nothing() {
    assert_eq!(walk_manifest(""), ManifestDeps::default());
    assert_eq!(walk_manifest("# only a comment\n"), ManifestDeps::default());
    // Unterminated header / quote: ignored rather than panicking.
    let m = "[dependencies.oxideav-png\nversion = \"0\"\n[target.'cfg(target_os = \"linux\").dependencies]\noxideav-vaapi = \"0\"\n";
    assert_eq!(walk_manifest(m), ManifestDeps::default());
}

#[test]
fn target_gates_table_is_whitespace_free_and_consistent() {
    for (key, emitted) in TARGET_GATES {
        assert!(!key.chars().any(char::is_whitespace), "{key:?}");
        let squeezed: String = emitted.chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(&squeezed, key, "emitted gate must squeeze to its key");
    }
}
