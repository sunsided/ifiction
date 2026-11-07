//! Property-based tests of the build helper: which IFID and defaults `build_story` picks,
//! how it turns description text into paragraphs, and what `add_build_info` changes.

use std::path::Path;

use ifiction::Ifid;
use ifiction::model::{Format, FormatSection, PartialDate, Release, Releases, Story};
use ifiction_build::{
    BuildInfo, Error, Metadata, PackageInfo, add_build_info, build_story, format_for_target,
};
use proptest::collection::vec;
use proptest::option::of as o;
use proptest::prelude::*;

fn ifid() -> impl Strategy<Value = Ifid> {
    "[0-9A-Z-]{8,63}".prop_map(|s| Ifid::parse(&s).expect("valid by construction"))
}

fn word() -> impl Strategy<Value = String> {
    "[A-Za-z0-9'.,!?é-]{1,10}"
}

/// Normalized single-line text that passes lint.
fn line() -> impl Strategy<Value = String> {
    vec(word(), 1..4).prop_map(|w| w.join(" "))
}

/// Paragraphs of words, and a text that spells them with line breaks and blank lines.
fn description() -> impl Strategy<Value = (String, String)> {
    vec(
        vec(
            (word(), prop_oneof![Just(" "), Just("\n"), Just("  \t")]),
            1..5,
        ),
        1..4,
    )
    .prop_flat_map(|paragraphs| {
        let gaps = vec(
            prop_oneof![Just("\n\n"), Just("\n  \n\n"), Just("\r\n\r\n")],
            0..=paragraphs.len(),
        );
        (Just(paragraphs), gaps, any::<bool>())
    })
    .prop_map(|(paragraphs, gaps, indent)| {
        let mut text = String::new();
        let mut expected = Vec::new();
        for (i, p) in paragraphs.iter().enumerate() {
            if i > 0 {
                text.push_str(gaps.get(i).copied().unwrap_or("\n\n"));
            }
            if indent {
                text.push_str("    ");
            }
            let words: Vec<&str> = p.iter().map(|(w, _)| w.as_str()).collect();
            for (j, (w, sep)) in p.iter().enumerate() {
                if j > 0 {
                    text.push_str(sep);
                }
                text.push_str(w);
            }
            expected.push(words.join(" "));
        }
        (text, expected.join("\n"))
    })
}

proptest! {
    /// The IFID comes from the entry of the format, in capitals; the defaults come from the
    /// package; description paragraphs are separated by blank lines.
    #[test]
    fn build_story_picks_and_defaults(
        exe in ifid(),
        web in ifid(),
        html in any::<bool>(),
        lower in any::<bool>(),
        title in o(line()),
        author in o(line()),
        name in line(),
        authors in vec((line(), o("[a-z]{1,6}@[a-z]{1,6}\\.org")), 0..3),
        (text, paragraphs) in description(),
        in_meta in any::<bool>(),
        year in 1970u16..=2100,
    ) {
        let spell = |i: &Ifid| {
            if lower { format!(" {}\n", i.as_str().to_ascii_lowercase()) } else { i.to_string() }
        };
        let mut meta = Metadata::default();
        meta.ifid.insert("executable".into(), spell(&exe));
        meta.ifid.insert("html".into(), spell(&web));
        meta.title.clone_from(&title);
        meta.author.clone_from(&author);
        let package = PackageInfo {
            name: name.clone(),
            authors: authors
                .iter()
                .map(|(n, mail)| match mail {
                    Some(m) => format!("{n} <{m}>"),
                    None => n.clone(),
                })
                .collect(),
            description: (!in_meta).then(|| text.clone()),
            homepage: None,
            version: "1.0.0".into(),
        };
        if in_meta {
            meta.description = Some(text);
        }
        let format = if html { Format::Html } else { Format::Executable };
        let day = PartialDate::Year(year);
        let built = build_story(Path::new("."), &meta, &package, format.clone(), day).unwrap();
        let s = &built.story;
        prop_assert_eq!(&s.identification.ifids, &vec![if html { web } else { exe }]);
        prop_assert_eq!(&s.identification.format, &format);
        prop_assert_eq!(&s.bibliographic.title, title.as_ref().unwrap_or(&name));
        let names: Vec<&str> = authors.iter().map(|(n, _)| n.as_str()).collect();
        let default_author = if names.is_empty() { "Anonymous".to_owned() } else { names.join(" and ") };
        prop_assert_eq!(&s.bibliographic.author, author.as_ref().unwrap_or(&default_author));
        prop_assert_eq!(s.bibliographic.description.as_deref(), Some(paragraphs.as_str()));
        let attached = s.releases.as_ref().and_then(|r| r.attached.as_ref()).unwrap();
        prop_assert_eq!(attached.release_date, day);
        prop_assert_eq!(s.colophon.as_ref().unwrap().originated, day);
    }

    /// A format without an IFID entry is an error, not a guess.
    #[test]
    fn build_story_needs_the_format_ifid(
        exe in ifid(),
        format in prop_oneof![Just(Format::Zcode), Just(Format::Glulx), Just(Format::Html), "x[a-z]{1,6}".prop_map(Format::Other)],
    ) {
        let mut meta = Metadata::default();
        meta.ifid.insert("executable".into(), exe.to_string());
        let package = PackageInfo { name: "game".into(), ..PackageInfo::default() };
        let result = build_story(Path::new("."), &meta, &package, format.clone(), PartialDate::Year(2026));
        prop_assert!(
            matches!(&result, Err(Error::MissingIfid { format: f }) if *f == format.to_string()),
            "{:?}", result.map(|b| b.story)
        );
    }

    /// Only `wasm*-*-unknown` is a web game.
    #[test]
    fn format_for_any_target(
        arch in prop_oneof!["wasm[0-9a-z]{0,4}", "[a-z0-9_]{1,8}"],
        os in prop_oneof![Just("unknown".to_owned()), "[a-z]{1,8}"],
    ) {
        let expected = if arch.starts_with("wasm") && os == "unknown" { Format::Html } else { Format::Executable };
        prop_assert_eq!(format_for_target(&arch, &os), expected);
    }

    /// `add_build_info` sets the target and the compiler, keeps every other entry, and is
    /// idempotent.
    #[test]
    fn add_build_info_keeps_entries(
        ifid in ifid(),
        format in prop_oneof![Just(Format::Executable), Just(Format::Html), Just(Format::Zcode)],
        entries in o(vec(("[a-z]{1,6}|target", line()), 0..4)),
        attached in any::<bool>(),
        target in o("[a-z0-9_]{1,8}-[a-z]{1,8}-[a-z]{1,8}"),
        rustc in o("1\\.[0-9]{1,3}\\.[0-9]"),
    ) {
        let mut story = Story::new(ifid, format.clone(), "T", "A");
        story.format_section = entries.clone().map(|entries| FormatSection {
            name: format.to_string(),
            entries,
        });
        story.releases = Some(Releases {
            attached: attached.then_some(Release {
                release_date: PartialDate::Year(2026),
                version: None,
                compiler: None,
                compiler_version: None,
            }),
            history: Vec::new(),
        });
        let build = BuildInfo { target: target.clone(), rustc_version: rustc.clone() };
        let before = story.clone();
        add_build_info(&mut story, &build);

        let mut twice = story.clone();
        add_build_info(&mut twice, &build);
        prop_assert_eq!(&twice, &story);

        let rel = story.releases.as_ref().unwrap().attached.as_ref();
        match (&rustc, rel) {
            (Some(v), Some(r)) => {
                prop_assert_eq!(r.compiler.as_deref(), Some("rustc"));
                prop_assert_eq!(r.compiler_version.as_ref(), Some(v));
            }
            _ => prop_assert_eq!(&story.releases, &before.releases),
        }

        match &target {
            Some(t) if matches!(format, Format::Executable | Format::Html) => {
                let section = story.format_section.as_ref().unwrap();
                prop_assert_eq!(&section.name, &format.to_string());
                let first = section.entries.iter().find(|(k, _)| k == "target");
                prop_assert_eq!(first.map(|(_, v)| v), Some(t));
                let others = |e: &[(String, String)]| -> Vec<(String, String)> {
                    e.iter().filter(|(k, _)| k != "target").cloned().collect()
                };
                prop_assert_eq!(others(&section.entries), others(&entries.unwrap_or_default()));
            }
            _ => prop_assert_eq!(&story.format_section, &before.format_section),
        }
    }
}
