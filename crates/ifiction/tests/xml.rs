//! Reading and writing whole records.

#![cfg(feature = "read")]

use ifiction::lint::{self, Severity};
use ifiction::model::*;
use ifiction::{Ifid, read, write};
use proptest::prelude::*;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).expect("fixture")
}

fn full_story() -> Story {
    let mut s = Story::new(
        Ifid::parse("448E73DF-2D2F-47E7-A494-A46B40D4CFB3").unwrap(),
        Format::Html,
        "The Lighthouse",
        "Markus Mayer",
    );
    let b = &mut s.bibliographic;
    b.language = Some("en".into());
    b.headline = Some("An Interactive Fiction".into());
    b.first_published = Some(PartialDate::Year(2026));
    b.genre = Some("Mystery".into());
    b.description = Some("First & <best> paragraph.\nSecond one.".into());
    b.series = Some("Coast".into());
    b.series_number = Some(1);
    b.forgiveness = Some(Forgiveness::Polite);
    s.resources.push(Auxiliary {
        leafname: "Manual.pdf".into(),
        description: "The manual".into(),
    });
    s.contacts = Some(Contacts {
        url: Some("https://example.org/lighthouse".into()),
        author_email: Some("me@example.org".into()),
    });
    s.cover = Some(Cover {
        format: CoverFormat::Png,
        width: 960,
        height: 960,
        description: Some("A lighthouse at night.".into()),
    });
    s.releases = Some(Releases {
        attached: Some(Release {
            release_date: "2026-11-07".parse().unwrap(),
            version: Some(2),
            compiler: Some("rustc".into()),
            compiler_version: Some("1.99.1".into()),
        }),
        history: vec![],
    });
    s.colophon = Some(Colophon {
        generator: "ifiction".into(),
        generator_version: Some("0.1.0".into()),
        originated: "2026-11-07".parse().unwrap(),
    });
    s
}

#[test]
fn round_trip_full_story() {
    let index = IfIndex::single(full_story());
    let xml = write::to_string(&index);
    assert!(xml.starts_with(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
    assert!(xml.contains(
        "<description>First &amp; &lt;best&gt; paragraph.<br/>Second one.</description>"
    ));
    let back = read::from_str(&xml).unwrap();
    assert_eq!(back, index);
    assert_eq!(write::to_string(&back), xml, "writing is deterministic");
    assert!(
        lint::check_index(&back)
            .iter()
            .all(|d| d.severity == Severity::Warning)
    );
}

#[test]
fn reads_legacy_record() {
    let index = read::from_str(&fixture("legacy.iFiction")).unwrap();
    assert_eq!(index.stories.len(), 2);
    let s = &index.stories[0];
    let ifids: Vec<&str> = s.identification.ifids.iter().map(Ifid::as_str).collect();
    assert_eq!(ifids, ["ZCODE-18-860904", "ZCODE-15-851108"]);
    assert_eq!(s.identification.format, Format::Zcode);
    assert_eq!(s.identification.bafn, Some(1234));
    assert_eq!(
        s.bibliographic.description.as_deref(),
        Some("You wake in a \u{201c}mine\u{201d} that is not a mine.\nTom & Jerry <3 you.")
    );
    assert_eq!(s.bibliographic.forgiveness, Some(Forgiveness::Tough));
    let history = &s.releases.as_ref().unwrap().history;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].compiler.as_deref(), Some("ZIL"));
    let zcode = s.format_section.as_ref().unwrap();
    assert_eq!(zcode.entries[2], ("serial".into(), "860904".into()));
    assert_eq!(index.stories[1].identification.format, Format::Glulx);
    assert_eq!(read::ifids(&fixture("legacy.iFiction")).unwrap().len(), 3);
}

#[test]
fn rejects_invalid_values() {
    let base = fixture("legacy.iFiction");
    for (from, to) in [
        ("<firstpublished>1986<", "<firstpublished>1986-09<"),
        ("<forgiveness>Tough<", "<forgiveness>tough<"),
        ("<ifid>ZCODE-18-860904<", "<ifid>zcode-18<"),
        ("<title>The Example Mine</title>", ""),
        ("<ifindex version", "<other version"),
    ] {
        let xml = base.replacen(from, to, 1);
        assert!(read::from_str(&xml).is_err(), "{to}");
    }
}

#[test]
fn lint_finds_problems() {
    let mut s = full_story();
    s.bibliographic.title = "THE  LIGHTHOUSE".into();
    s.bibliographic.language = Some("English".into());
    s.bibliographic.series = None;
    s.bibliographic.genre = Some("Cozy".into());
    s.cover.as_mut().unwrap().width = 100;
    s.resources[0].leafname = "docs/manual.v2.pdf".into();
    let found: Vec<String> = lint::check(&s).iter().map(ToString::to_string).collect();
    for expected in [
        "error: bibliographic/title: must not contain runs of spaces",
        "warning: bibliographic/title: should not be in capitals",
        "error: bibliographic/language",
        "error: bibliographic/seriesnumber: needs <series>",
        "warning: bibliographic/genre",
        "error: cover: 100x960: each side must be at least 120 pixels",
        "error: cover: 100x960: the aspect ratio must be between 1:2 and 2:1",
        "error: resources/auxiliary[1]/leafname: is a file name without a path",
        "error: resources/auxiliary[1]/leafname: has at most one full stop",
    ] {
        assert!(
            found.iter().any(|f| f.starts_with(expected)),
            "missing {expected:?} in {found:#?}"
        );
    }
}

proptest! {
    #[test]
    fn text_survives_round_trip(title in "[A-Za-z0-9&<>\"' .,:;!?é-]{1,60}") {
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        prop_assume!(!title.is_empty());
        let mut s = full_story();
        s.bibliographic.title = title.clone();
        let xml = write::to_string(&IfIndex::single(s));
        let back = read::from_str(&xml).unwrap();
        prop_assert_eq!(&back.stories[0].bibliographic.title, &title);
    }

    #[test]
    fn valid_ifids_parse(s in "[0-9A-Z-]{8,63}") {
        prop_assert!(Ifid::parse(&s).is_ok());
        let marker = Ifid::parse(&s).unwrap().marker();
        let found = ifiction::scan::find_markers(marker.as_bytes());
        prop_assert_eq!(found.len(), 1);
    }
}

#[test]
fn br_outside_description_is_a_space() {
    // Found by fuzzing: <br/> used to become a newline in every element, which the writer
    // then wrote as text, so a write and read changed the record.
    let xml = "<ifindex><story><identification><ifid>ZCODE-1-2-3</ifid><format>zcode</format></identification><bibliographic><title>A<br/>B</title><author>C</author><description>One<br/>Two</description></bibliographic></story></ifindex>";
    let index = read::from_str(xml).unwrap();
    let b = &index.stories[0].bibliographic;
    assert_eq!(b.title, "A B");
    assert_eq!(b.description.as_deref(), Some("One\nTwo"));
    assert_eq!(read::from_str(&write::to_string(&index)).unwrap(), index);
}
