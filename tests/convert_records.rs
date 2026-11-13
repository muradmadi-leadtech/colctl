//! Cycle 7, T3: how a conversion is recorded. `convert = ["codex"]` on a `[[source]]` (written when
//! approved), and a held conversion in the state (docs/convert/interfaces.md § Config, § State).

use colctl::config::{self, Source};
use colctl::state::{self, Candidate, HeldConversion};
use colctl::tool::Tool;
use std::fs;
use std::path::Path;

const REV: &str = "0123456789abcdef0123456789abcdef01234567";

fn text(extra: &str) -> String {
    format!(
        "version = 1\n\n[[source]]\nname = \"qdrant-skills\"\nrepo = \"https://github.com/qdrant/skills\"\nrev = \"{REV}\"\nplugin = \"qdrant\"\n{extra}"
    )
}

fn parse(text: &str) -> Result<config::Config, String> {
    config::parse(text, Path::new("/home/me"))
}

#[test]
fn a_source_may_convert_for_codex() {
    let c = parse(&text("convert = [\"codex\"]\n")).unwrap();
    assert_eq!(c.sources[0].convert, vec![Tool::Codex]);
    let c = parse(&text("")).unwrap();
    assert!(c.sources[0].convert.is_empty());
}

#[test]
fn only_codex_can_be_a_conversion_target() {
    for bad in [
        "convert = [\"claude\"]",
        "convert = [\"cursor\"]",
        "convert = \"codex\"",
    ] {
        let err = parse(&text(&format!("{bad}\n"))).unwrap_err();
        assert!(err.contains("convert"), "{bad}: {err}");
    }
}

#[test]
fn approving_writes_convert_and_undo_removes_it() {
    let base = text("");
    let on = config::set_source_convert(&base, "qdrant-skills", true).unwrap();
    assert!(on.contains("convert = [\"codex\"]"), "{on}");
    assert_eq!(parse(&on).unwrap().sources[0].convert, vec![Tool::Codex]);
    // Once.
    assert_eq!(
        config::set_source_convert(&on, "qdrant-skills", true).unwrap(),
        on
    );
    let off = config::set_source_convert(&on, "qdrant-skills", false).unwrap();
    assert!(!off.contains("convert"), "{off}");
    assert!(config::set_source_convert(&base, "nope", true).is_err());
}

#[test]
fn a_new_source_keeps_its_convert() {
    let source = Source {
        name: "q".into(),
        repo: "https://github.com/qdrant/skills".into(),
        rev: REV.into(),
        plugin: "q".into(),
        replaces: vec![],
        path: None,
        convert: vec![Tool::Codex],
    };
    let out = config::add_source("version = 1\n", &source).unwrap();
    assert_eq!(parse(&out).unwrap().sources[0], source);
}

#[test]
fn a_held_conversion_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = state::State::default();
    let held = Candidate::Conversion(HeldConversion {
        rev: REV.into(),
        digest: "a".repeat(64),
        held_at: 1_763_050_000,
    });
    assert_eq!(held.kind(), "conversion");
    assert_eq!(held.version(), "a".repeat(64));
    s.quarantine
        .insert("conversions/qdrant-skills".into(), held.clone());
    state::save(tmp.path(), &s).unwrap();
    let text = fs::read_to_string(tmp.path().join(".state/state.json")).unwrap();
    assert!(text.contains("\"kind\": \"conversion\""), "{text}");
    assert_eq!(
        state::load(tmp.path()).unwrap().quarantine["conversions/qdrant-skills"],
        held
    );
}

#[test]
fn a_held_conversion_elsewhere_is_corrupt() {
    let tmp = tempfile::tempdir().unwrap();
    let write = |key: &str, rev: &str, digest: &str| {
        fs::create_dir_all(tmp.path().join(".state")).unwrap();
        fs::write(
            tmp.path().join(".state/state.json"),
            format!(
                r#"{{"version": 1, "quarantine": {{"{key}": {{"kind": "conversion", "rev": "{rev}", "digest": "{digest}", "held_at": 1}}}}}}"#
            ),
        )
        .unwrap();
    };
    let digest = "b".repeat(64);
    write("conversions/q", REV, &digest);
    assert!(state::load(tmp.path()).is_ok());
    for (key, rev, digest) in [
        ("sources/q", REV, digest.as_str()),
        ("conversions/q", "short", digest.as_str()),
        ("conversions/q", REV, "nothex"),
    ] {
        write(key, rev, digest);
        assert!(state::load(tmp.path()).is_err(), "{key} {rev} {digest}");
    }
}
