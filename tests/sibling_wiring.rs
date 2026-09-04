//! Per-sibling wiring smoke tests + registry-collision audit.
//!
//! `register_all_smoke.rs` only proves the generated dispatch shell
//! compiles and populates *something*. This file goes one step deeper
//! for the siblings wired in round 456 (`mov`, `riff`, `tta`, `svq`):
//! each `#[cfg(feature = "...")]` block asserts that the sibling's
//! concrete claims — container name / extension / content probe, codec
//! id / FourCC tag — resolve through the aggregated `RuntimeContext`
//! exactly as they do when the sibling is registered alone. That is
//! the property a consumer of `oxideav-meta` actually relies on: a
//! later sibling in `register_all`'s alphabetical order must not
//! shadow an earlier one's claim.
//!
//! The collision-audit tests at the bottom lock the resolution rules
//! documented in the README ("Resolution order") for every feature
//! set they run under:
//!
//! - codec FourCC tags: highest probe confidence, ties → first
//!   registration (= alphabetically-first crate) wins;
//! - payload magics: longest prefix, ties → first registration;
//! - container content probes: highest score; a tie is *unspecified*
//!   (the registry walks a `HashMap`), so the audit builds several
//!   fresh registries and asserts the winner never changes for the
//!   synthetic inputs below — a flake here means two siblings score
//!   the same input equally and one of them must yield. Ties that are
//!   already known (`ftyp qt  `: `mov` vs `mp4`) are pinned in
//!   `KNOWN_PROBE_TIES` and only required to stay inside their set;
//! - container names / extensions: last registration wins.

use std::io::Cursor;

use oxideav_core::RuntimeContext;

// ─────────────────────── synthetic inputs ───────────────────────

/// Minimal ISO-BMFF `ftyp` box with the given major brand and
/// compatible-brand list, followed by an empty `free` box so the
/// buffer is comfortably longer than any probe's minimum length.
fn ftyp(major: &[u8; 4], compat: &[&[u8; 4]]) -> Vec<u8> {
    let size = 16 + 4 * compat.len();
    let mut out = Vec::with_capacity(size + 64);
    out.extend_from_slice(&(size as u32).to_be_bytes());
    out.extend_from_slice(b"ftyp");
    out.extend_from_slice(major);
    out.extend_from_slice(&0u32.to_be_bytes()); // minor version
    for c in compat {
        out.extend_from_slice(*c);
    }
    out.extend_from_slice(&64u32.to_be_bytes());
    out.extend_from_slice(b"free");
    out.resize(out.len() + 56, 0);
    out
}

/// QuickTime-branded `ftyp` (`qt  ` major brand) — the QTFF signature
/// `oxideav-mov` claims outright.
fn qt_ftyp() -> Vec<u8> {
    ftyp(b"qt  ", &[b"qt  "])
}

/// Generic ISO-BMFF `ftyp` (`isom` major brand, no `qt  ` anywhere) —
/// `oxideav-mov` must yield this one to `oxideav-mp4`.
fn isom_ftyp() -> Vec<u8> {
    ftyp(b"isom", &[b"isom", b"mp41"])
}

/// `TTA1` magic followed by zeros — enough for the True Audio
/// container probe (which only inspects the 4-byte magic).
fn tta1_head() -> Vec<u8> {
    let mut out = b"TTA1".to_vec();
    out.resize(64, 0);
    out
}

/// A complete RIFF/WAVE file: 16-byte PCM `fmt ` chunk + 4-byte
/// `data` chunk (the smallest well-formed WAV the RIFF spec allows).
fn riff_wave() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&0u32.to_le_bytes()); // patched below
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // WAVE_FORMAT_PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // channels
    out.extend_from_slice(&8000u32.to_le_bytes()); // sample rate
    out.extend_from_slice(&16000u32.to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    let riff_size = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff_size.to_le_bytes());
    out
}

/// Every synthetic input the container-probe audit walks, with a
/// label for diagnostics.
fn probe_corpus() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("ftyp/qt", qt_ftyp()),
        ("ftyp/isom", isom_ftyp()),
        ("TTA1", tta1_head()),
        ("RIFF/WAVE", riff_wave()),
    ]
}

fn ctx_all() -> RuntimeContext {
    let mut ctx = RuntimeContext::new();
    oxideav_meta::register_all(&mut ctx);
    ctx
}

fn probe_name(ctx: &RuntimeContext, bytes: &[u8]) -> Option<String> {
    let mut cur = Cursor::new(bytes.to_vec());
    ctx.containers.probe_input(&mut cur, None).ok()
}

// ─────────────────────────── mov ───────────────────────────

#[cfg(feature = "mov")]
mod mov {
    use super::*;

    #[test]
    fn mov_is_dispatched_by_register_all() {
        assert!(
            oxideav_meta::ENABLED_SIBLINGS.contains(&("oxideav-mov", "mov")),
            "mov feature is on but ENABLED_SIBLINGS lacks oxideav-mov: {:?}",
            oxideav_meta::ENABLED_SIBLINGS,
        );
        assert_eq!(oxideav_meta::category_of("mov"), Some("container"));
    }

    #[test]
    fn mov_container_claims_survive_register_all() {
        // What the sibling claims when registered alone …
        let mut solo = RuntimeContext::new();
        oxideav_mov::registry::register(&mut solo);
        assert!(solo.containers.demuxer_names().any(|n| n == "mov"));
        assert!(solo.containers.muxer_names().any(|n| n == "mov"));
        assert_eq!(solo.containers.container_for_extension("mov"), Some("mov"));
        assert_eq!(solo.containers.container_for_extension("qt"), Some("mov"));

        // … must still resolve identically after every other enabled
        // sibling has registered (names / extensions are last-wins).
        let all = ctx_all();
        assert!(all.containers.demuxer_names().any(|n| n == "mov"));
        assert!(all.containers.muxer_names().any(|n| n == "mov"));
        assert_eq!(all.containers.container_for_extension("mov"), Some("mov"));
        assert_eq!(all.containers.container_for_extension("qt"), Some("mov"));
    }

    #[test]
    fn mov_probe_claims_qt_branded_ftyp() {
        let all = ctx_all();
        let got = probe_name(&all, &qt_ftyp());
        if cfg!(feature = "mp4") {
            // KNOWN TIE (see `KNOWN_PROBE_TIES`): `oxideav-mp4` scores a
            // `qt  `-branded ftyp as high as `oxideav-mov` does, and the
            // core registry breaks probe ties in `HashMap` order — so
            // with both siblings enabled the winner is one of the two,
            // not necessarily `mov`. Tightened to `== "mov"` once mp4
            // yields `qt  ` brands (or core breaks ties by registration
            // order).
            assert!(
                matches!(got.as_deref(), Some("mov") | Some("mp4")),
                "qt-branded ftyp resolved to {got:?}; expected mov or mp4",
            );
        } else {
            assert_eq!(
                got.as_deref(),
                Some("mov"),
                "a `qt  `-branded ftyp must probe as the QTFF container",
            );
        }
    }

    #[cfg(feature = "mp4")]
    #[test]
    fn mov_yields_generic_isom_ftyp_to_mp4() {
        // The one documented probe overlap: both QTFF and ISO-BMFF
        // start with `ftyp`. `oxideav-mov` scores 0 unless a `qt  `
        // brand is present, so a plain `isom` file must NOT come back
        // as `mov`.
        let all = ctx_all();
        let got = probe_name(&all, &isom_ftyp());
        assert_ne!(got.as_deref(), Some("mov"), "mov must yield non-QT ftyp");
        assert_eq!(
            got.as_deref(),
            Some("mp4"),
            "generic isom ftyp resolves to mp4"
        );
    }
}

// ─────────────────────────── tta ───────────────────────────

#[cfg(feature = "tta")]
mod tta {
    use super::*;
    use oxideav_core::CodecId;

    #[test]
    fn tta_is_dispatched_by_register_all() {
        assert!(oxideav_meta::ENABLED_SIBLINGS.contains(&("oxideav-tta", "tta")));
        assert_eq!(oxideav_meta::category_of("tta"), Some("audio-codec"));
    }

    #[test]
    fn tta_codec_and_container_claims_survive_register_all() {
        let id = CodecId::new(oxideav_tta::CODEC_ID_STR);
        assert_eq!(oxideav_tta::CODEC_ID_STR, "tta");

        let mut solo = RuntimeContext::new();
        oxideav_tta::register(&mut solo);
        assert!(solo.codecs.has_decoder(&id));
        assert!(solo.codecs.has_encoder(&id));
        assert_eq!(solo.containers.container_for_extension("tta"), Some("tta"));

        let all = ctx_all();
        assert!(
            all.codecs.has_decoder(&id),
            "tta decoder missing after register_all"
        );
        assert!(
            all.codecs.has_encoder(&id),
            "tta encoder missing after register_all"
        );
        assert!(all.containers.demuxer_names().any(|n| n == "tta"));
        assert_eq!(all.containers.container_for_extension("tta"), Some("tta"));
    }

    #[test]
    fn tta1_magic_probes_as_tta() {
        let all = ctx_all();
        assert_eq!(probe_name(&all, &tta1_head()).as_deref(), Some("tta"));
    }
}

// ─────────────────────────── svq ───────────────────────────

#[cfg(feature = "svq")]
mod svq {
    use super::*;
    use oxideav_core::{CodecId, CodecTag, ProbeContext};

    #[test]
    fn svq_is_dispatched_by_register_all() {
        assert!(oxideav_meta::ENABLED_SIBLINGS.contains(&("oxideav-svq", "svq")));
        assert_eq!(oxideav_meta::category_of("svq"), Some("video-codec"));
    }

    #[test]
    fn svq_codec_ids_survive_register_all() {
        let svq1 = CodecId::new(oxideav_svq::CODEC_ID_STR);
        let svq3 = CodecId::new(oxideav_svq::SVQ3_CODEC_ID_STR);
        let all = ctx_all();
        assert!(all.codecs.has_decoder(&svq1), "svq1 decoder missing");
        assert!(all.codecs.has_encoder(&svq1), "svq1 encoder missing");
        assert!(all.codecs.has_decoder(&svq3), "svq3 decoder missing");
    }

    #[test]
    fn svq_fourcc_tags_resolve_through_the_aggregate_registry() {
        let all = ctx_all();
        let svq1 = CodecId::new(oxideav_svq::CODEC_ID_STR);
        let svq3 = CodecId::new(oxideav_svq::SVQ3_CODEC_ID_STR);

        // Every FourCC the sibling publishes for each id, plus the
        // lower-case QuickTime spelling (`CodecTag::fourcc` folds
        // case) — each must be claimed by exactly one codec id and
        // resolve to the Sorenson decoder even without packet bytes.
        for raw in oxideav_svq::SVQ1_FOURCC_CODES {
            let tag = CodecTag::fourcc(raw);
            let claimants: Vec<_> = all
                .codecs
                .all_tag_registrations()
                .filter(|(t, _)| **t == tag)
                .map(|(_, id)| id.clone())
                .collect();
            assert_eq!(claimants, vec![svq1.clone()], "tag {tag:?} claimants");
            assert_eq!(
                all.codecs.resolve_tag_ref(&ProbeContext::new(&tag)),
                Some(&svq1),
                "tag {tag:?} must resolve to svq1",
            );
        }
        for raw in oxideav_svq::SVQ3_FOURCC_CODES {
            let tag = CodecTag::fourcc(raw);
            let claimants: Vec<_> = all
                .codecs
                .all_tag_registrations()
                .filter(|(t, _)| **t == tag)
                .map(|(_, id)| id.clone())
                .collect();
            assert_eq!(claimants, vec![svq3.clone()], "tag {tag:?} claimants");
            assert_eq!(
                all.codecs.resolve_tag_ref(&ProbeContext::new(&tag)),
                Some(&svq3),
                "tag {tag:?} must resolve to svq3",
            );
        }
    }
}

// ─────────────────────────── riff ───────────────────────────

#[cfg(feature = "riff")]
mod riff {
    use super::*;
    use oxideav_riff::{Walker, FOURCC_RIFF};

    #[test]
    fn riff_is_library_only() {
        // Pulled into the dependency tree by the `riff` feature but
        // never dispatched: no register entry point exists in the
        // published crate.
        assert!(
            oxideav_meta::ENABLED_LIBRARY_ONLY_SIBLINGS.contains(&("oxideav-riff", "riff")),
            "riff feature is on but ENABLED_LIBRARY_ONLY_SIBLINGS lacks it: {:?}",
            oxideav_meta::ENABLED_LIBRARY_ONLY_SIBLINGS,
        );
        assert!(
            !oxideav_meta::ENABLED_SIBLINGS
                .iter()
                .any(|(_, s)| *s == "riff"),
            "riff must not appear in ENABLED_SIBLINGS (nothing is dispatched for it)",
        );
        assert_eq!(oxideav_meta::category_of("riff"), None);
    }

    #[test]
    fn riff_walker_reaches_the_dependency_tree() {
        // The crate's public chunk walker resolves against the
        // published API: open the RIFF root, read the two children.
        let bytes = riff_wave();
        let mut cur = Cursor::new(bytes);
        let mut w = Walker::open_root(&mut cur).expect("open RIFF root");
        let fmt = w.read_next().expect("read fmt").expect("fmt present");
        assert_eq!(&fmt.id, b"fmt ");
        assert_eq!(fmt.size, 16);
        assert!(!fmt.is_group());
        w.skip(&fmt).expect("skip fmt body");
        let data = w.read_next().expect("read data").expect("data present");
        assert_eq!(&data.id, b"data");
        assert_eq!(data.size, 4);
        assert_eq!(w.read_body(&data).expect("read data body"), [0, 0, 0, 0]);
        assert_eq!(w.remaining(), 0);
        assert_eq!(w.read_next().expect("clean EOF"), None);
        assert_eq!(FOURCC_RIFF, *b"RIFF");
    }

    #[test]
    fn riff_wave_is_not_claimed_by_riff_itself() {
        // Whatever container claims a RIFF/WAVE file through the
        // aggregate registry, it is never `oxideav-riff` — the walker
        // has no demuxer to register. (The existing wav-family
        // siblings, if enabled, may legitimately claim it.)
        let all = ctx_all();
        assert_ne!(probe_name(&all, &riff_wave()).as_deref(), Some("riff"));
        assert!(!all.containers.demuxer_names().any(|n| n == "riff"));
    }
}

// ───────────────────── introspection consistency ─────────────────────

#[test]
fn library_only_siblings_are_disjoint_from_enabled_siblings() {
    for (full, short) in oxideav_meta::ENABLED_LIBRARY_ONLY_SIBLINGS {
        assert!(
            !oxideav_meta::ENABLED_SIBLINGS_ALL
                .iter()
                .any(|(f, _)| f == full),
            "{full} is listed both as library-only and as a dispatched sibling",
        );
        assert_eq!(
            oxideav_meta::category_of(short),
            None,
            "library-only sibling {short} must have no register_all category",
        );
        assert_eq!(full.strip_prefix("oxideav-"), Some(*short));
    }
    let names: Vec<&str> = oxideav_meta::ENABLED_LIBRARY_ONLY_SIBLINGS
        .iter()
        .map(|(f, _)| *f)
        .collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(
        names, sorted,
        "ENABLED_LIBRARY_ONLY_SIBLINGS must be alphabetical"
    );
}

// ───────────────────────── collision audit ─────────────────────────

/// Codec FourCC tags claimed by more than one codec id. A collision is
/// legitimate (several codecs can share a tag and let their probes
/// disambiguate) as long as the aggregate registry still resolves the
/// tag to one of its claimants. The audit also prints the collision
/// set so the README's "Resolution order" table can be kept honest.
#[test]
fn codec_tag_collisions_resolve_to_a_claimant() {
    use oxideav_core::{CodecId, CodecTag, ProbeContext};
    use std::collections::BTreeMap;

    let all = ctx_all();
    let mut claims: BTreeMap<String, (CodecTag, Vec<CodecId>)> = BTreeMap::new();
    for (tag, id) in all.codecs.all_tag_registrations() {
        let e = claims
            .entry(format!("{tag:?}"))
            .or_insert_with(|| (tag.clone(), Vec::new()));
        if !e.1.contains(id) {
            e.1.push(id.clone());
        }
    }
    let mut collisions = 0;
    for (label, (tag, ids)) in &claims {
        if ids.len() < 2 {
            continue;
        }
        collisions += 1;
        let resolved = all.codecs.resolve_tag_ref(&ProbeContext::new(tag));
        eprintln!("tag collision {label}: claimants {ids:?} -> resolves to {resolved:?}");
        if let Some(r) = resolved {
            assert!(
                ids.contains(r),
                "tag {label} resolved to {r:?}, which is not one of its claimants {ids:?}",
            );
        }
    }
    eprintln!(
        "{collisions} codec-tag collisions across {} distinct tags",
        claims.len()
    );
}

/// Payload magics: the same duplicate-claim audit for the byte-prefix
/// index. Identical magics claimed by two ids are a real ambiguity
/// (there is no probe step to break the tie — first registration
/// wins silently), so those are reported loudly and must not exist.
#[test]
fn payload_magic_claims_are_unambiguous() {
    use std::collections::BTreeMap;

    let all = ctx_all();
    let mut by_magic: BTreeMap<Vec<u8>, Vec<String>> = BTreeMap::new();
    for (magic, id) in all.codecs.all_payload_magic_registrations() {
        let e = by_magic.entry(magic.to_vec()).or_default();
        let s = id.as_str().to_string();
        if !e.contains(&s) {
            e.push(s);
        }
    }
    let dupes: Vec<_> = by_magic.iter().filter(|(_, ids)| ids.len() > 1).collect();
    assert!(
        dupes.is_empty(),
        "identical payload magic claimed by several codec ids (first registration wins \
         silently — one of them must drop or lengthen its claim): {dupes:?}",
    );
}

/// Content-probe ties the audit already knows about. Each entry names
/// a `probe_corpus` label, the feature pair that has to be enabled for
/// the tie to exist, and the container names that share the top
/// score. The core registry breaks probe ties in `HashMap` order, so
/// for these inputs the winner is *any* of the listed names — the
/// determinism test below only requires that it never leaves the set.
///
/// - `ftyp/qt`: `oxideav-mp4` scores a `qt  `-branded `ftyp` as high
///   as `oxideav-mov` (which returns the maximum score for it), so a
///   QuickTime file may open as either container. `oxideav-mov`
///   already yields non-QT brands to `mp4`; the symmetric yield on the
///   mp4 side (or a registration-order tie-break in core) is what
///   removes this entry.
const KNOWN_PROBE_TIES: &[(&str, (bool, bool), &[&str])] = &[(
    "ftyp/qt",
    (cfg!(feature = "mov"), cfg!(feature = "mp4")),
    &["mov", "mp4"],
)];

fn known_tie_for(label: &str) -> Option<&'static [&'static str]> {
    KNOWN_PROBE_TIES
        .iter()
        .find(|(l, (a, b), _)| *l == label && *a && *b)
        .map(|(_, _, names)| *names)
}

/// Container probes: the registry keeps the highest score and breaks
/// ties in `HashMap` order, so a tie between two siblings is a
/// non-deterministic result. Build several independent registries and
/// require the winner to be identical every time for each synthetic
/// input above — except for the ties in `KNOWN_PROBE_TIES`, where the
/// winner only has to stay inside the documented set.
#[test]
fn container_probe_winner_is_stable_across_fresh_registries() {
    const ROUNDS: usize = 12;
    let corpus = probe_corpus();
    for (label, bytes) in &corpus {
        let first = probe_name(&ctx_all(), bytes);
        let tie = known_tie_for(label);
        for round in 1..ROUNDS {
            let again = probe_name(&ctx_all(), bytes);
            match tie {
                Some(names) => {
                    let inside = again.as_deref().is_some_and(|n| names.contains(&n));
                    assert!(
                        inside,
                        "probe winner for {label} was {again:?} (round {round}); \
                         expected one of the known tie set {names:?}",
                    );
                }
                None => assert_eq!(
                    again, first,
                    "probe winner for {label} changed between fresh registries (round {round}): \
                     two container probes tie on this input and one must yield",
                ),
            }
        }
        eprintln!("probe {label}: {first:?} (known tie: {tie:?})");
    }
}
