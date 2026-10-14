//! `Cargo.toml` dependency walker shared by `build.rs` and the
//! `tests/build_script_parser.rs` harness (both pull this file in with
//! `#[path = "…/build/manifest.rs"] mod manifest;`).
//!
//! The build script must find every `oxideav-*` dependency in **both**
//! manifest shapes:
//!
//! * the in-tree shape — inline entries under a section header:
//!
//!   ```toml
//!   [dependencies]
//!   oxideav-png = { version = "0", optional = true }
//!
//!   [target.'cfg(target_os = "macos")'.dependencies]
//!   oxideav-videotoolbox = { version = "0", optional = true }
//!   ```
//!
//! * the normalized shape cargo writes into the published `.crate`
//!   (and that every crates.io consumer builds from) — one table per
//!   dependency, comments gone, keys possibly double-quoted:
//!
//!   ```toml
//!   [dependencies.oxideav-png]
//!   version = "0"
//!   optional = true
//!
//!   [target.'cfg(target_os = "macos")'.dependencies.oxideav-videotoolbox]
//!   version = "0"
//!   optional = true
//!   ```
//!
//! `oxideav-meta` 0.0.2 only understood the first shape, so
//! `register_all` was empty for every crates.io consumer. Standing rule:
//! **this parser keeps understanding the normalized manifest**, the
//! `tests/build_script_parser.rs` fixtures lock both shapes, and the
//! `off-umbrella` CI job proves it from a packaged `.crate`.
//!
//! Hand-written on purpose: a build-dependency on a TOML crate would be
//! the only non-oxideav dependency of this crate, and the subset of TOML
//! needed here (table headers with quoted dotted keys, `key = …` lines)
//! is small and fixed.

/// Crates we depend on but that should NOT be in `register_all` (see
/// also `LIBRARY_ONLY` in `build.rs` for siblings that are skipped but
/// still surfaced through `ENABLED_LIBRARY_ONLY_SIBLINGS`):
/// - `oxideav-core` is the foundation, not a register-emitting sibling.
/// - The 3D crates (`oxideav-mesh3d` + the format siblings) use a
///   separate dispatch contract: they register into a
///   `oxideav_mesh3d::Mesh3DRegistry` via `populate_mesh3d_registry`,
///   not into the codec/container/filter/source RuntimeContext that
///   `register_all` walks.
pub const SKIP: &[&str] = &[
    "oxideav-core",
    "oxideav-mesh3d",
    "oxideav-stl",
    "oxideav-obj",
    "oxideav-gltf",
    "oxideav-usdz",
    "oxideav-fbx",
    "oxideav-ifc",
    "oxideav-vrml",
    "oxideav-x3d",
    "oxideav-step",
];

/// Recognized `[target.'cfg(…)'.dependencies]` gates as
/// `(cfg body with all whitespace removed, cfg body as emitted)`. The
/// first column is what a header's `cfg(…)` argument is compared
/// against after whitespace stripping — so `'cfg(target_os="macos")'`,
/// `'cfg(target_os = "macos")'` and the double-quoted
/// `"cfg(target_os = \"macos\")"` all match the same row. The second
/// column is emitted verbatim as `#[cfg(…)]` on the generated
/// `register_all` call.
pub const TARGET_GATES: &[(&str, &str)] = &[
    ("target_os=\"macos\"", "target_os = \"macos\""),
    ("target_os=\"linux\"", "target_os = \"linux\""),
    (
        "any(target_os=\"linux\",target_os=\"windows\")",
        "any(target_os = \"linux\", target_os = \"windows\")",
    ),
];

/// Everything the walker learned about the manifest's `oxideav-*`
/// dependencies. `deps` is what `register_all` is generated from; the
/// other fields let `build.rs` fail loud when the manifest shape drifts
/// instead of silently emitting an empty `register_all` again.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ManifestDeps {
    /// `(crate name, cfg gate)` for every dependency `register_all`
    /// may dispatch — `[dependencies]` plus the recognized target gates,
    /// minus [`SKIP`]. `None` gate = plain `[dependencies]`; `Some(s)`
    /// = emit `#[cfg(s)]` on the call. Sorted by crate name (then gate),
    /// deduplicated, so the output is deterministic whatever order the
    /// manifest lists them in.
    pub deps: Vec<(String, Option<String>)>,
    /// `oxideav-*` dependencies that were seen but are in [`SKIP`]
    /// (`oxideav-core`, the 3D crates). Sorted + deduplicated. The build
    /// script requires `oxideav-core` here: it is the one dependency
    /// every manifest shape must contain, so its absence means the
    /// parser understood nothing.
    pub skipped: Vec<String>,
    /// `oxideav-*` dependencies found under a `[target.….dependencies]`
    /// header whose gate is not in [`TARGET_GATES`], as
    /// `(header text, crate name)`. The build script fails loud on any
    /// entry so a sibling behind a new gate gets a `TARGET_GATES` row
    /// instead of vanishing from `register_all`.
    pub unrecognized: Vec<(String, String)>,
}

/// Where the walker is inside the manifest.
enum Section {
    /// Lines are not dependency entries (`[package]`, `[features]`,
    /// `[dev-dependencies]`, the body of a `[dependencies.x]` table, an
    /// unrecognized target header, …).
    Inert,
    /// `[dependencies]` / `[target.'cfg(…)'.dependencies]` with a
    /// recognized gate: every `key = …` line is a dependency.
    Deps(Option<&'static str>),
    /// Same, but under a target header whose gate is unknown: `key = …`
    /// lines are reported, not emitted.
    UnrecognizedDeps(String),
}

/// Walk the manifest for `oxideav-*` dependencies in either shape; see
/// [`ManifestDeps`] for what comes back. `build.rs` generates
/// `register_all` from `.deps` and fails loud on the diagnostics.
pub fn walk_manifest(manifest: &str) -> ManifestDeps {
    let mut out = ManifestDeps::default();
    let mut section = Section::Inert;

    for raw in manifest.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') {
            section = Section::Inert;
            let Some(header) = header_text(line) else {
                continue;
            };
            match classify_header(&header) {
                Header::DepSection(gate) => section = Section::Deps(gate),
                Header::DepTable(gate, name) => record(&mut out, &name, gate),
                Header::UnrecognizedSection => section = Section::UnrecognizedDeps(header),
                Header::UnrecognizedTable(name) => out.unrecognized.push((header, name)),
                Header::Other => {}
            }
            continue;
        }

        let Some(eq) = line.find('=') else {
            continue;
        };
        let key = unquote_key(line[..eq].trim());
        match &section {
            Section::Inert => {}
            Section::Deps(gate) => record(&mut out, &key, *gate),
            Section::UnrecognizedDeps(header) => {
                if key.starts_with("oxideav-") {
                    out.unrecognized.push((header.clone(), key));
                }
            }
        }
    }

    out.deps.sort();
    out.deps.dedup();
    out.skipped.sort();
    out.skipped.dedup();
    out.unrecognized.sort();
    out.unrecognized.dedup();
    out
}

fn record(out: &mut ManifestDeps, name: &str, gate: Option<&'static str>) {
    if !name.starts_with("oxideav-") {
        return;
    }
    if SKIP.contains(&name) {
        out.skipped.push(name.to_string());
    } else {
        out.deps.push((name.to_string(), gate.map(str::to_string)));
    }
}

/// Classification of a `[…]` table header.
enum Header {
    /// `[dependencies]` or a recognized `[target.'cfg(…)'.dependencies]`.
    DepSection(Option<&'static str>),
    /// `[dependencies.NAME]` or a recognized
    /// `[target.'cfg(…)'.dependencies.NAME]`.
    DepTable(Option<&'static str>, String),
    /// `[target.X.dependencies]` with an unknown gate / target triple.
    UnrecognizedSection,
    /// `[target.X.dependencies.NAME]` with an unknown gate.
    UnrecognizedTable(String),
    /// Anything else, including `[[test]]` arrays of tables and
    /// `[dev-dependencies]` / `[build-dependencies]`.
    Other,
}

fn classify_header(header: &str) -> Header {
    if header.starts_with('[') {
        return Header::Other; // `[[test]]` — array of tables, never deps.
    }
    let Some(parts) = split_dotted_key(header) else {
        return Header::Other;
    };
    let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["dependencies"] => Header::DepSection(None),
        ["dependencies", name] => Header::DepTable(None, (*name).to_string()),
        ["target", cfg, "dependencies"] => match gate_for(cfg) {
            Some(gate) => Header::DepSection(Some(gate)),
            None => Header::UnrecognizedSection,
        },
        ["target", cfg, "dependencies", name] => match gate_for(cfg) {
            Some(gate) => Header::DepTable(Some(gate), (*name).to_string()),
            None => Header::UnrecognizedTable((*name).to_string()),
        },
        _ => Header::Other,
    }
}

/// Map a target key (`cfg(target_os = "macos")`, already unquoted) to
/// the gate body to emit, or `None` for target triples / unknown gates.
fn gate_for(target_key: &str) -> Option<&'static str> {
    let squeezed: String = target_key.chars().filter(|c| !c.is_whitespace()).collect();
    let body = squeezed.strip_prefix("cfg(")?.strip_suffix(')')?;
    TARGET_GATES
        .iter()
        .find(|(key, _)| *key == body)
        .map(|(_, emit)| *emit)
}

/// The text between a header line's opening `[` and its closing `]`,
/// honouring quoted segments (a `]` inside `'…'` / `"…"` does not
/// close the header) and ignoring any trailing comment. `[[…]]` arrays
/// of tables come back with their inner `[` so the caller can reject
/// them. `None` if the bracket is never closed.
fn header_text(line: &str) -> Option<String> {
    let inner = line.strip_prefix('[')?;
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in inner.chars() {
        match quote {
            Some(q) => {
                out.push(c);
                if escaped {
                    escaped = false;
                } else if q == '"' && c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    out.push(c);
                }
                ']' => return Some(out.trim().to_string()),
                _ => out.push(c),
            },
        }
    }
    None
}

/// Split a dotted TOML key into its (unquoted, unescaped) segments:
/// bare keys, single-quoted literal strings (no escapes) and
/// double-quoted basic strings (`\"` and `\\` escapes). Whitespace
/// around the dots is ignored. `None` on malformed input (unterminated
/// quote, empty segment).
fn split_dotted_key(key: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut chars = key.chars().peekable();
    // Whether the current segment came from a quoted string (so an
    // empty quoted key is still a segment — cargo never writes one, but
    // the distinction keeps the parser honest).
    let mut quoted = false;
    loop {
        match chars.next() {
            None => {
                if cur.is_empty() && !quoted {
                    return None;
                }
                parts.push(cur);
                return Some(parts);
            }
            Some('.') => {
                if cur.is_empty() && !quoted {
                    return None;
                }
                parts.push(std::mem::take(&mut cur));
                quoted = false;
            }
            Some(c) if c.is_whitespace() => {}
            Some('\'') => {
                quoted = true;
                loop {
                    match chars.next() {
                        None => return None,
                        Some('\'') => break,
                        Some(c) => cur.push(c),
                    }
                }
            }
            Some('"') => {
                quoted = true;
                loop {
                    match chars.next() {
                        None => return None,
                        Some('"') => break,
                        Some('\\') => cur.push(chars.next()?),
                        Some(c) => cur.push(c),
                    }
                }
            }
            Some(c) => cur.push(c),
        }
    }
}

/// The dependency name of a `key = …` line: one layer of quotes
/// stripped (`"oxideav-png"` → `oxideav-png`), and for a dotted key
/// (`oxideav-png.version = "0"`, which TOML reads as
/// `[dependencies.oxideav-png] version = "0"`) the first segment. Bare
/// keys pass through unchanged.
fn unquote_key(key: &str) -> String {
    split_dotted_key(key)
        .and_then(|mut parts| (!parts.is_empty()).then(|| parts.remove(0)))
        .unwrap_or_else(|| key.to_string())
}
