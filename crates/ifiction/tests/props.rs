//! Property-based tests: invariants of IFIDs, dates, the writer and reader, lint, HTML
//! injection, detection and cover probing, checked on generated inputs.
//!
//! Tests that need the `read` or `detect` feature live in modules gated on them, so the
//! file also builds with `--no-default-features`.

use ifiction::lint::{self, Severity};
use ifiction::model::*;
use ifiction::{Ifid, IfidError, cover, html, scan, write};
use proptest::collection::vec;
use proptest::option::of as o;
use proptest::prelude::*;

// ---------------------------------------------------------------------------------------
// Strategies and helpers
// ---------------------------------------------------------------------------------------

/// A valid IFID.
fn ifid() -> impl Strategy<Value = Ifid> {
    "[0-9A-Z-]{8,63}".prop_map(|s| Ifid::parse(&s).expect("valid by construction"))
}

/// A word without whitespace, with the characters the writer must escape.
fn word() -> impl Strategy<Value = String> {
    "[A-Za-z0-9&<>\"'.,:;!?()/é日-]{1,10}"
}

/// Normalized single-line text: words separated by single spaces.
fn line() -> impl Strategy<Value = String> {
    vec(word(), 1..5).prop_map(|w| w.join(" "))
}

fn days_in_month(year: u16, month: u8) -> u8 {
    let leap = (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
    match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A valid date; the last day of a month and February come up often.
fn valid_date() -> impl Strategy<Value = (u16, u8, u8)> {
    (0u16..=9999, prop_oneof![Just(2u8), 1u8..=12], any::<bool>()).prop_flat_map(|(y, m, last)| {
        let dim = days_in_month(y, m);
        let day = if last {
            Just(dim).boxed()
        } else {
            (1..=dim).boxed()
        };
        (Just(y), Just(m), day)
    })
}

fn partial_date() -> impl Strategy<Value = PartialDate> {
    prop_oneof![
        (0u16..=9999).prop_map(PartialDate::Year),
        valid_date().prop_map(|(year, month, day)| PartialDate::Date { year, month, day }),
    ]
}

fn forgiveness() -> impl Strategy<Value = Forgiveness> {
    prop_oneof![
        Just(Forgiveness::Merciful),
        Just(Forgiveness::Polite),
        Just(Forgiveness::Tough),
        Just(Forgiveness::Nasty),
        Just(Forgiveness::Cruel),
    ]
}

/// A Treaty format, or an element-name-safe other one that collides with no section.
fn format() -> impl Strategy<Value = Format> {
    prop_oneof![
        Just(Format::Zcode),
        Just(Format::Glulx),
        Just(Format::Tads3),
        Just(Format::Html),
        Just(Format::Executable),
        "x[a-z0-9]{1,8}".prop_map(Format::Other),
    ]
}

fn release(text: fn() -> BoxedStrategy<String>) -> impl Strategy<Value = Release> {
    (partial_date(), o(any::<u32>()), o(text()), o(text())).prop_map(
        |(release_date, version, compiler, compiler_version)| Release {
            release_date,
            version,
            compiler,
            compiler_version,
        },
    )
}

/// A story with every optional section, its text drawn from `text` and `para`.
///
/// `text` gives single-line values, `para` gives description paragraphs.
fn story_with(
    text: fn() -> BoxedStrategy<String>,
    para: fn() -> BoxedStrategy<String>,
) -> impl Strategy<Value = Story> {
    let identification = (vec(ifid(), 1..3), format(), o(text()), o(any::<u32>()));
    let bibliographic = (
        (text(), text(), o(text()), o(text()), o(partial_date())),
        (
            o(text()),
            o(text()),
            o(vec(para(), 1..4)),
            o(text()),
            o(any::<u32>()),
            o(forgiveness()),
        ),
    );
    let rest = (
        vec((text(), text()), 0..3),
        o((o(text()), o(text()))),
        o((any::<bool>(), any::<u32>(), any::<u32>(), o(text()))),
        o((o(release(text)), vec(release(text), 0..3))),
        o((text(), o(text()), partial_date())),
        o(vec(
            (
                "[a-z][a-z0-9]{0,8}",
                prop_oneof![text(), Just(String::new())],
            ),
            0..4,
        )),
    );
    (identification, bibliographic, rest).prop_map(|(id, bib, rest)| {
        let (ifids, format, tuid, bafn) = id;
        let ((title, author, language, headline, first_published), more) = bib;
        let (genre, group, description, series, series_number, forgiveness) = more;
        let (resources, contacts, cover, releases, colophon, section) = rest;
        Story {
            format_section: section.map(|entries| FormatSection {
                name: format.as_str().to_owned(),
                entries,
            }),
            identification: Identification {
                ifids,
                format,
                tuid,
                bafn,
            },
            bibliographic: Bibliographic {
                title,
                author,
                language,
                headline,
                first_published,
                genre,
                group,
                description: description.map(|p| p.join("\n")),
                series,
                series_number,
                forgiveness,
            },
            resources: resources
                .into_iter()
                .map(|(leafname, description)| Auxiliary {
                    leafname,
                    description,
                })
                .collect(),
            contacts: contacts.map(|(url, author_email)| Contacts { url, author_email }),
            cover: cover.map(|(png, width, height, description)| Cover {
                format: if png {
                    CoverFormat::Png
                } else {
                    CoverFormat::Jpg
                },
                width,
                height,
                description,
            }),
            releases: releases.map(|(attached, history)| Releases { attached, history }),
            colophon: colophon.map(|(generator, generator_version, originated)| Colophon {
                generator,
                generator_version,
                originated,
            }),
        }
    })
}

/// A story that follows every rule of the Treaty: lint finds no errors.
fn nice_story() -> impl Strategy<Value = Story> {
    let lang = "[a-z]{2,3}(-[A-Z]{2})?";
    let release = (partial_date(), o(any::<u32>()), o((line(), o(line()))))
        .prop_map(|(release_date, version, compiler)| Release {
            release_date,
            version,
            compiler: compiler.as_ref().map(|(c, _)| c.clone()),
            compiler_version: compiler.and_then(|(_, v)| v),
        })
        .boxed();
    let cover = (120u32..=1200).prop_flat_map(|w| {
        let lo = w.div_ceil(2).max(120);
        let hi = (2 * w).min(1200);
        (Just(w), lo..=hi, any::<bool>(), o(line()))
    });
    (
        (ifid(), format(), o("[A-Za-z0-9]{1,16}"), o(any::<u32>())),
        (line(), line(), o(lang), o(line()), o(partial_date())),
        (
            o(line()),
            o(vec(line(), 1..4)),
            o((line(), o(any::<u32>()))),
        ),
        vec(("[A-Za-z0-9_-]{1,8}(\\.[a-z]{1,4})?", line()), 0..3),
        o((
            o("https?://[a-z]{1,10}\\.org/[a-z]{0,6}"),
            o(vec("[a-z]{1,6}@[a-z]{1,6}\\.org", 1..3)),
        )),
        o(cover),
        o((o(release.clone()), vec(release, 0..3))),
        o((line(), partial_date())),
        o(vec(("[a-z][a-z0-9]{0,8}", line()), 0..3)),
    )
        .prop_map(
            |(id, bib, more, resources, contacts, cover, releases, colophon, section)| {
                let (ifid, format, tuid, bafn) = id;
                let (title, author, language, headline, first_published) = bib;
                let (group, description, series) = more;
                let mut s = Story::new(ifid, format.clone(), title, author);
                s.identification.tuid = tuid;
                s.identification.bafn = bafn;
                let b = &mut s.bibliographic;
                b.language = language;
                b.headline = headline;
                b.first_published = first_published;
                b.group = group;
                b.description = description.map(|p| p.join("\n"));
                b.series_number = series.as_ref().and_then(|(_, n)| *n);
                b.series = series.map(|(s, _)| s);
                s.resources = resources
                    .into_iter()
                    .map(|(leafname, description)| Auxiliary {
                        leafname,
                        description,
                    })
                    .collect();
                s.contacts = contacts.map(|(url, mail)| Contacts {
                    url,
                    author_email: mail.map(|m| m.join(", ")),
                });
                s.cover = cover.map(|(width, height, png, description)| Cover {
                    format: if png {
                        CoverFormat::Png
                    } else {
                        CoverFormat::Jpg
                    },
                    width,
                    height,
                    description,
                });
                s.releases = releases.map(|(attached, history)| Releases { attached, history });
                s.colophon = colophon.map(|(generator, originated)| Colophon {
                    generator,
                    generator_version: None,
                    originated,
                });
                s.format_section = section.map(|entries| FormatSection {
                    name: format.as_str().to_owned(),
                    entries,
                });
                s
            },
        )
}

/// Undoes [`write::escape`].
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// A well-formed HTML page, and the IFID meta tag it already has, if any.
fn html_page() -> impl Strategy<Value = (String, Option<String>)> {
    (
        "[ \t]{0,4}",
        prop_oneof![Just("<head>"), Just("<HEAD>"), Just("<head lang=\"en\">")],
        any::<bool>(),
        any::<bool>(),
        o(ifid()),
        vec(word().prop_map(|w| w.replace(['<', '>', '&'], "")), 0..4),
        line(),
    )
        .prop_map(|(indent, head, newlines, charset, old, titles, body)| {
            let nl = if newlines { "\n" } else { "" };
            let inner = if newlines {
                format!("{indent}  ")
            } else {
                String::new()
            };
            let mut h = format!("<!doctype html>{nl}<html>{nl}{indent}{head}");
            if charset {
                h.push_str(&format!("{nl}{inner}<meta charset=\"utf-8\">"));
            }
            let old_tag = old.map(|i| format!(r#"<meta property="ifiction:ifid" content="{i}">"#));
            if let Some(t) = &old_tag {
                h.push_str(&format!("{nl}{inner}{t}"));
            }
            for t in titles {
                h.push_str(&format!("{nl}{inner}<title>{t}</title>"));
            }
            let body = write::escape(&body);
            h.push_str(&format!(
                "{nl}{indent}</head>{nl}{indent}<body><p>{body}</p></body>{nl}</html>{nl}"
            ));
            (h, old_tag)
        })
}

/// Soup of HTML fragments: unbalanced quotes, stray tags, odd case.
fn html_soup() -> impl Strategy<Value = String> {
    vec(
        prop_oneof![
            Just("<head>"),
            Just("<HEAD "),
            Just("<header>"),
            Just("<meta "),
            Just("<meta charset=utf-8>"),
            Just("property=\"ifiction:ifid\""),
            Just("property=ifiction:ifid"),
            Just("content='ABCDEFGH'"),
            Just("UUID://ABCDEFGH//"),
            Just("\""),
            Just("'"),
            Just(">"),
            Just("<"),
            Just("/"),
            Just("="),
            Just("\n"),
            Just("\r\n"),
            Just(" "),
            Just("\t"),
            Just("a"),
            Just("é"),
        ],
        0..24,
    )
    .prop_map(|parts| parts.concat())
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    png.extend(w.to_be_bytes());
    png.extend(h.to_be_bytes());
    png.extend([8, 6, 0, 0, 0]);
    png
}

/// Days from 1970-01-01 to a date, counted the slow way.
fn days_since_epoch(year: u16, month: u8, day: u8) -> i64 {
    let year_len = |y: u16| if days_in_month(y, 2) == 29 { 366 } else { 365 };
    let mut days: i64 = if year >= 1970 {
        (1970..year).map(year_len).sum()
    } else {
        -(year..1970).map(year_len).sum::<i64>()
    };
    days += (1..month)
        .map(|m| i64::from(days_in_month(year, m)))
        .sum::<i64>();
    days + i64::from(day) - 1
}

// ---------------------------------------------------------------------------------------
// IFIDs and markers
// ---------------------------------------------------------------------------------------

proptest! {
    /// `parse` accepts exactly 8 to 63 digits, capitals and hyphens, and says why not.
    #[test]
    fn ifid_parse_is_exact(s in prop_oneof!["[0-9A-Z-]{6,65}", "[0-9A-Za-z_ é-]{0,70}"]) {
        let len = s.chars().count();
        let bad = s.chars().find(|c| !(c.is_ascii_digit() || c.is_ascii_uppercase() || *c == '-'));
        let expected = if !(8..=63).contains(&len) {
            Err(IfidError::Length(len))
        } else if let Some(c) = bad {
            Err(IfidError::Char(c))
        } else {
            Ok(s.clone())
        };
        prop_assert_eq!(Ifid::parse(&s).map(String::from), expected);
    }

    /// `parse_lenient` trims and capitalizes, then parses strictly.
    #[test]
    fn ifid_parse_lenient(
        ifid in ifid(),
        lead in "[ \t\n]{0,3}",
        trail in "[ \t\n]{0,3}",
        other in "[0-9A-Za-z_ é-]{0,70}",
    ) {
        let lower = format!("{lead}{}{trail}", ifid.as_str().to_ascii_lowercase());
        prop_assert_eq!(Ifid::parse_lenient(&lower), Ok(ifid));
        prop_assert_eq!(
            Ifid::parse_lenient(&other),
            Ifid::parse(&other.trim().to_ascii_uppercase())
        );
    }

    /// Markers are found in order, without duplicates, between arbitrary text.
    #[test]
    fn markers_round_trip(
        parts in vec((ifid(), "[a-z \0\n<>:]{0,8}"), 0..5),
        lead in "[a-z \0\n<>:]{0,8}",
    ) {
        let mut bytes = lead.into_bytes();
        let mut expected: Vec<Ifid> = Vec::new();
        for (ifid, junk) in &parts {
            bytes.extend(ifid.marker().bytes());
            bytes.extend(junk.bytes());
            if !expected.contains(ifid) {
                expected.push(ifid.clone());
            }
        }
        prop_assert_eq!(scan::find_markers(&bytes), expected);
    }

    /// A marker with a lowercase letter does not count, as in babel.
    #[test]
    fn lowercase_markers_are_ignored(
        s in "[0-9A-Za-z-]{8,63}".prop_filter("has lowercase", |s| s.bytes().any(|b| b.is_ascii_lowercase())),
    ) {
        let text = format!("UUID://{s}//");
        prop_assert!(scan::find_markers(text.as_bytes()).is_empty());
    }

    #[test]
    fn find_markers_never_panics(bytes in vec(any::<u8>(), 0..256), tail in "[A-Z/:-]{0,12}") {
        let mut bytes = bytes;
        bytes.extend(b"UUID://");
        bytes.extend(tail.bytes());
        for ifid in scan::find_markers(&bytes) {
            prop_assert!(Ifid::parse(ifid.as_str()).is_ok());
        }
    }
}

// ---------------------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------------------

proptest! {
    /// A `YYYY-MM-DD` string parses exactly when it is a calendar day, and prints back.
    #[test]
    fn date_parse_is_exact(y in 0u16..=9999, m in prop_oneof![1u8..=12, 0u8..=99], d in 0u8..=32) {
        let s = format!("{y:04}-{m:02}-{d:02}");
        let valid = (1..=12).contains(&m) && (1..=days_in_month(y, m)).contains(&d);
        let parsed = s.parse::<PartialDate>();
        prop_assert_eq!(parsed.is_ok(), valid, "{}", s);
        if let Ok(date) = parsed {
            prop_assert_eq!(date, PartialDate::Date { year: y, month: m, day: d });
            prop_assert_eq!(date.to_string(), s);
        }
    }

    /// Display and `FromStr` are inverse.
    #[test]
    fn date_display_round_trips(date in partial_date()) {
        prop_assert_eq!(date.to_string().parse::<PartialDate>(), Ok(date));
    }

    /// Whatever parses is in canonical form: printing it gives the same string.
    #[test]
    fn parsed_dates_are_canonical(s in "[0-9-]{0,12}") {
        if let Ok(date) = s.parse::<PartialDate>() {
            prop_assert_eq!(date.to_string(), s);
        }
    }

    /// Any second of a day from year 0 to 9999 maps to that day.
    #[test]
    fn unix_seconds_match_calendar((y, m, d) in valid_date(), secs in 0i64..86_400) {
        let days = days_since_epoch(y, m, d);
        prop_assert_eq!(
            PartialDate::from_unix_seconds(days * 86_400 + secs),
            PartialDate::Date { year: y, month: m, day: d }
        );
    }

    /// Any timestamp, also far outside years 0 to 9999, gives a date the reader accepts.
    #[test]
    fn unix_seconds_always_valid(
        secs in prop_oneof![
            any::<i64>(),
            (10000u16..=12000, prop_oneof![Just(2u8), 1u8..=12], 1u8..=31)
                .prop_map(|(y, m, d)| {
                    let d = d.min(days_in_month(y, m));
                    // Past 9999, count from 9999-12-31 with the slow reference.
                    let base = days_since_epoch(9999, 12, 31) + 1;
                    let extra: i64 = (10000..y)
                        .map(|y| if days_in_month(y, 2) == 29 { 366 } else { 365 })
                        .sum::<i64>()
                        + (1..m).map(|m| i64::from(days_in_month(y, m))).sum::<i64>()
                        + i64::from(d)
                        - 1;
                    (base + extra) * 86_400
                }),
        ],
    ) {
        let date = PartialDate::from_unix_seconds(secs);
        prop_assert_eq!(date.to_string().parse::<PartialDate>(), Ok(date), "secs = {}", secs);
    }
}

// ---------------------------------------------------------------------------------------
// Writer and lint
// ---------------------------------------------------------------------------------------

proptest! {
    /// `escape` leaves no `<` or `>` and only the three entities of the Treaty.
    #[test]
    fn escape_uses_three_entities(s in any::<String>()) {
        let e = write::escape(&s);
        prop_assert!(!e.contains('<') && !e.contains('>'));
        for (i, _) in e.match_indices('&') {
            let rest = &e[i..];
            prop_assert!(
                rest.starts_with("&amp;") || rest.starts_with("&lt;") || rest.starts_with("&gt;"),
                "{}", e
            );
        }
        prop_assert_eq!(unescape(&e), s);
    }

    /// Lint never panics, whatever the text of a record.
    #[test]
    fn lint_never_panics(
        mut story in story_with(|| any::<String>().boxed(), || any::<String>().boxed()),
    ) {
        // Cover sizes from 2^31 up overflow; `lint_cover_never_panics` covers them.
        if let Some(c) = &mut story.cover {
            c.width %= 1 << 31;
            c.height %= 1 << 31;
        }
        let _ = lint::check(&story);
        let _ = lint::check_index(&IfIndex { stories: vec![story.clone(), story] });
    }

    /// Lint never panics on any cover size, and flags the extreme ones.
    #[test]
    fn lint_cover_never_panics(width in any::<u32>(), height in any::<u32>()) {
        let mut story = Story::new(Ifid::parse("ABCDEFGH").unwrap(), Format::Html, "T", "A");
        story.cover = Some(Cover { format: CoverFormat::Png, width, height, description: None });
        let found = lint::check(&story);
        let bad = width < 120 || height < 120
            || u64::from(width) > 2 * u64::from(height)
            || u64::from(height) > 2 * u64::from(width);
        prop_assert_eq!(
            found.iter().any(|d| d.severity == Severity::Error && d.path == "cover"),
            bad
        );
    }

    /// A record that follows the rules has no errors (warnings are allowed).
    #[test]
    fn nice_records_have_no_errors(story in nice_story()) {
        let errors: Vec<String> = lint::check(&story)
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(ToString::to_string)
            .collect();
        prop_assert!(errors.is_empty(), "{:#?}", errors);
    }

    /// Breaking one rule of a valid record gives an error at that element.
    #[test]
    fn broken_rules_are_errors(mut story in nice_story(), rule in 0usize..10, w in "[a-z]{1,8}") {
        let path = match rule {
            0 => {
                story.bibliographic.title = format!("{w}  {w}");
                "bibliographic/title".to_owned()
            }
            1 => {
                story.bibliographic.author = format!("\t{w}");
                "bibliographic/author".to_owned()
            }
            2 => {
                story.bibliographic.language = Some("English".into());
                "bibliographic/language".to_owned()
            }
            3 => {
                story.bibliographic.series = None;
                story.bibliographic.series_number = Some(1);
                "bibliographic/seriesnumber".to_owned()
            }
            4 => {
                story.resources.push(Auxiliary {
                    leafname: format!("docs/{w}.pdf"),
                    description: w,
                });
                format!("resources/auxiliary[{}]/leafname", story.resources.len())
            }
            5 => {
                story.cover = Some(Cover {
                    format: CoverFormat::Png,
                    width: 119,
                    height: 200,
                    description: None,
                });
                "cover".to_owned()
            }
            6 => {
                story.releases = Some(Releases {
                    attached: Some(Release {
                        release_date: PartialDate::Year(2026),
                        version: None,
                        compiler: None,
                        compiler_version: Some(w),
                    }),
                    history: vec![],
                });
                "releases/release[1]/compilerversion".to_owned()
            }
            7 => {
                story.contacts = Some(Contacts {
                    url: Some(format!("{w}.org")),
                    author_email: None,
                });
                "contacts/url".to_owned()
            }
            8 => {
                story.identification.tuid = Some(format!("{w}-{w}"));
                "identification/tuid".to_owned()
            }
            _ => {
                story.identification.ifids.clear();
                "identification/ifid".to_owned()
            }
        };
        let found = lint::check(&story);
        prop_assert!(
            found.iter().any(|d| d.severity == Severity::Error && d.path == path),
            "no error at {} in {:#?}", path, found
        );
    }
}

// ---------------------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------------------

proptest! {
    /// On a well-formed page, injection is idempotent, the IFID reads back, and nothing but
    /// the tag changes.
    #[test]
    fn inject_well_formed((page, old) in html_page(), ifid in ifid()) {
        let tag = html::meta_tag(&ifid);
        let out = html::inject(&page, &ifid).unwrap();
        prop_assert_eq!(html::inject(&out, &ifid).unwrap(), out.clone());
        prop_assert_eq!(html::extract_ifid(&out), Some((ifid, html::HtmlSource::Meta)));
        prop_assert_eq!(out.matches("ifiction:ifid").count(), 1);
        match old {
            Some(old) => prop_assert_eq!(&out, &page.replacen(&old, &tag, 1)),
            None => {
                // The tag goes on its own line: a newline, the indent, the tag.
                let at = out.find(&tag).unwrap();
                let nl = out[..at].rfind('\n').unwrap();
                prop_assert!(out[nl + 1..at].chars().all(|c| c == ' ' || c == '\t'));
                let restored = format!("{}{}", &out[..nl], &out[at + tag.len()..]);
                prop_assert_eq!(restored, page);
            }
        }
    }

    /// On any text, injection succeeds or reports a missing `<head>`, and never panics.
    #[test]
    fn inject_never_panics(s in prop_oneof![any::<String>(), html_soup()], ifid in ifid()) {
        match html::inject(&s, &ifid) {
            Ok(out) => prop_assert!(out.contains(&html::meta_tag(&ifid))),
            Err(e) => prop_assert_eq!(e, html::HtmlError::NoHead),
        }
        let _ = html::extract_ifid(&s);
    }

    /// The documented guarantee "injecting the same IFID twice gives the same document",
    /// and that the injected IFID is the one read back, on malformed HTML.
    #[test]
    fn inject_soup_is_idempotent(s in html_soup(), ifid in ifid()) {
        if let Ok(out) = html::inject(&s, &ifid) {
            prop_assert_eq!(html::inject(&out, &ifid).unwrap(), out.clone(), "input {:?}", s);
            prop_assert_eq!(
                html::extract_ifid(&out),
                Some((ifid, html::HtmlSource::Meta)),
                "input {:?}", s
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Cover images
// ---------------------------------------------------------------------------------------

proptest! {
    #[test]
    fn probe_never_panics(
        prefix in prop_oneof![
            Just(&b""[..]),
            Just(&b"\x89PNG\r\n\x1a\n"[..]),
            Just(&b"\xff\xd8"[..]),
            Just(&b"\xff\xd8\xff"[..]),
        ],
        rest in vec(prop_oneof![any::<u8>(), Just(0xffu8), 0xc0u8..=0xcf], 0..64),
    ) {
        let mut bytes = prefix.to_vec();
        bytes.extend(rest);
        let _ = cover::probe(&bytes);
    }

    #[test]
    fn probe_png(w in any::<u32>(), h in any::<u32>(), tail in vec(any::<u8>(), 0..16)) {
        let mut bytes = png(w, h);
        bytes.extend(tail);
        prop_assert_eq!(cover::probe(&bytes), Ok((CoverFormat::Png, w, h)));
    }

    /// The size comes from the first start-of-frame, after any other segments.
    #[test]
    fn probe_jpeg(
        w in any::<u16>(),
        h in any::<u16>(),
        segments in vec((0xe0u8..=0xef, vec(any::<u8>(), 0..20), 0usize..3), 0..4),
        sof in (0xc0u8..=0xcf).prop_filter("start of frame", |m| ![0xc4, 0xc8, 0xcc].contains(m)),
    ) {
        let mut bytes = vec![0xff, 0xd8];
        for (marker, payload, fill) in segments {
            bytes.extend(std::iter::repeat_n(0xff, fill));
            bytes.extend([0xff, marker]);
            bytes.extend(u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
            bytes.extend(payload);
        }
        bytes.extend([0xff, sof, 0x00, 0x11, 0x08]);
        bytes.extend(h.to_be_bytes());
        bytes.extend(w.to_be_bytes());
        bytes.extend([0x03, 0x01, 0x22, 0x00]);
        prop_assert_eq!(cover::probe(&bytes), Ok((CoverFormat::Jpg, u32::from(w), u32::from(h))));
    }
}

// ---------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------

#[cfg(feature = "read")]
mod read_props {
    use super::*;
    use ifiction::read;

    /// Text with at least one word and arbitrary whitespace runs, also at both ends.
    fn messy() -> impl Strategy<Value = String> {
        let ws = || "[ \t\r\n\u{a0}\u{2003}]{0,3}";
        (
            ws(),
            vec((word(), "[ \t\r\n\u{a0}\u{2003}]{1,3}"), 1..5),
            ws(),
        )
            .prop_map(|(lead, words, trail)| {
                let mut s = lead;
                for (w, sep) in words {
                    s.push_str(&w);
                    s.push_str(&sep);
                }
                s.push_str(&trail);
                s
            })
    }

    /// Possibly empty text of whitespace and words.
    fn messy_or_blank() -> impl Strategy<Value = String> {
        prop_oneof![messy(), "[ \t\n]{0,3}"]
    }

    /// Whitespace runs collapsed to single spaces, trimmed.
    fn collapse(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn collapse_opt(s: &Option<String>) -> Option<String> {
        s.as_deref().map(collapse).filter(|s| !s.is_empty())
    }

    /// What the reader gives back for a story: the documented normalizations of the text.
    fn normalize(mut s: Story) -> Story {
        let id = &mut s.identification;
        id.tuid = collapse_opt(&id.tuid);
        let b = &mut s.bibliographic;
        b.title = collapse(&b.title);
        b.author = collapse(&b.author);
        for v in [
            &mut b.language,
            &mut b.headline,
            &mut b.genre,
            &mut b.group,
            &mut b.series,
        ] {
            *v = collapse_opt(v);
        }
        b.description = b.description.as_deref().and_then(|d| {
            let paragraphs: Vec<String> = d
                .split('\n')
                .map(collapse)
                .filter(|p| !p.is_empty())
                .collect();
            (!paragraphs.is_empty()).then(|| paragraphs.join("\n"))
        });
        for a in &mut s.resources {
            a.leafname = collapse(&a.leafname);
            a.description = collapse(&a.description);
        }
        if let Some(c) = &mut s.contacts {
            c.url = collapse_opt(&c.url);
            c.author_email = collapse_opt(&c.author_email);
        }
        if let Some(c) = &mut s.cover {
            c.description = collapse_opt(&c.description);
        }
        if let Some(r) = &mut s.releases {
            for rel in r.attached.iter_mut().chain(&mut r.history) {
                rel.compiler = collapse_opt(&rel.compiler);
                rel.compiler_version = collapse_opt(&rel.compiler_version);
            }
        }
        if let Some(c) = &mut s.colophon {
            c.generator = collapse(&c.generator);
            c.generator_version = collapse_opt(&c.generator_version);
        }
        if let Some(f) = &mut s.format_section {
            for (_, v) in &mut f.entries {
                *v = collapse(v);
            }
        }
        s
    }

    fn line_boxed() -> BoxedStrategy<String> {
        line().boxed()
    }

    fn messy_boxed() -> BoxedStrategy<String> {
        messy().boxed()
    }

    fn messy_or_blank_boxed() -> BoxedStrategy<String> {
        messy_or_blank().boxed()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// Writing then reading gives the record back, with whitespace normalized.
        #[test]
        fn write_read_round_trip(
            stories in vec(story_with(messy_boxed, messy_or_blank_boxed), 1..3),
        ) {
            let index = IfIndex { stories };
            let xml = write::to_string(&index);
            let back = read::from_str(&xml).unwrap();
            let expected = IfIndex {
                stories: index.stories.into_iter().map(normalize).collect(),
            };
            prop_assert_eq!(&back, &expected);
            // The normalized record is a fixed point, so writing is deterministic.
            prop_assert_eq!(write::to_string(&back), write::to_string(&expected));
            prop_assert_eq!(read::from_str(&write::to_string(&back)).unwrap(), back);
        }

        /// A record of normalized text comes back unchanged, byte for byte.
        #[test]
        fn normalized_records_are_fixed_points(story in story_with(line_boxed, line_boxed)) {
            let index = IfIndex::single(story);
            let xml = write::to_string(&index);
            let back = read::from_str(&xml).unwrap();
            prop_assert_eq!(&back, &index);
            prop_assert_eq!(write::to_string(&back), xml);
            prop_assert_eq!(read::ifids(&write::to_string(&back)).unwrap(), index.stories[0].identification.ifids.clone());
        }

        /// Damaged records are an error or a record, never a panic.
        #[test]
        fn read_damaged_records(
            story in nice_story(),
            cut in any::<prop::sample::Index>(),
            len in 0usize..40,
            insert in prop_oneof![
                Just(""), Just("<"), Just(">"), Just("</story>"), Just("<br/>"), Just("&"),
                Just("&nbsp;"), Just("&#xD800;"), Just("&#0;"), Just("<![CDATA["), Just("]]>"),
                Just("<!--"), Just("\u{feff}"), Just("<ifindex>"),
            ],
        ) {
            let xml = write::to_string(&IfIndex::single(story));
            let mut at = cut.index(xml.len());
            while !xml.is_char_boundary(at) {
                at -= 1;
            }
            let mut end = (at + len).min(xml.len());
            while !xml.is_char_boundary(end) {
                end -= 1;
            }
            let damaged = format!("{}{insert}{}", &xml[..at], &xml[end..]);
            let _ = read::from_str(&damaged);
            let _ = read::ifids(&damaged);
            let _ = scan::find_ifiction(damaged.as_bytes());
        }

        #[test]
        fn read_never_panics(s in any::<String>()) {
            let _ = read::from_str(&s);
            let _ = read::ifids(&s);
            let _ = scan::find_ifiction(s.as_bytes());
        }
    }
}

// ---------------------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------------------

#[cfg(feature = "detect")]
mod detect_props {
    use super::*;
    use ifiction::detect::{self, FileKind, Source};

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// Any bytes give at least one valid IFID, the same each time.
        #[test]
        fn detect_never_panics(
            prefix in prop_oneof![
                Just(&b""[..]), Just(&b"\x7fELF"[..]), Just(&b"MZ"[..]), Just(&b"<html><head>"[..]),
                Just(&b"<?xml version=\"1.0\"?><ifindex>"[..]), Just(&b"\xca\xfe\xba\xbe"[..]),
            ],
            rest in vec(any::<u8>(), 0..256),
        ) {
            let mut bytes = prefix.to_vec();
            bytes.extend(rest);
            let d = detect::detect(&bytes);
            prop_assert!(!d.ifids.is_empty());
            for ifid in &d.ifids {
                prop_assert!(Ifid::parse(ifid.as_str()).is_ok());
            }
            prop_assert_eq!(detect::detect(&bytes), d);
        }

        /// In a file that is neither HTML nor iFiction, the first marker wins.
        #[test]
        fn detect_finds_marker(
            ifid in ifid(),
            magic in prop_oneof![Just(&b""[..]), Just(&b"\x7fELF"[..]), Just(&b"MZ"[..]), Just(&b"\0asm"[..])],
            before in vec(any::<u8>(), 0..128),
            after in vec(any::<u8>(), 0..128),
        ) {
            let mut bytes = magic.to_vec();
            bytes.extend(&before);
            prop_assume!(!contains(&bytes, b"UUID://"));
            bytes.extend(ifid.marker().bytes());
            bytes.extend(after);
            let d = detect::detect(&bytes);
            prop_assume!(!matches!(d.kind, FileKind::Html | FileKind::IFiction));
            prop_assert_eq!(d.source, Source::Marker);
            prop_assert_eq!(&d.ifids[0], &ifid);
        }

        /// The fallback is the babel prefix (or `HTML-`) and the MD5 in capitals.
        #[test]
        fn fallback_shape(
            magic in prop_oneof![
                Just(&b""[..]), Just(&b"MZ"[..]), Just(&b"\x7fELF"[..]), Just(&b"#!"[..]),
                Just(&b"\xca\xfe\xba\xbe"[..]), Just(&b"\0\0\x03\xe7"[..]),
                Just(&b"\xfe\xed\xfa\xce"[..]), Just(&b"APPL"[..]),
            ],
            rest in vec(any::<u8>(), 0..64),
            html in any::<bool>(),
        ) {
            let mut bytes = magic.to_vec();
            bytes.extend(rest);
            let kind = if html { FileKind::Html } else { FileKind::sniff(&bytes) };
            let ifid = detect::fallback_ifid(&bytes, kind);
            prop_assert_eq!(&detect::fallback_ifid(&bytes, kind), &ifid);
            let prefix = match kind {
                FileKind::Html => "HTML-",
                FileKind::IFiction => "",
                _ => detect::babel_prefix(&bytes),
            };
            let hex = ifid.as_str().strip_prefix(prefix).unwrap();
            prop_assert_eq!(hex.len(), 32);
            prop_assert!(hex.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b)));
        }

        /// A written record is detected as iFiction, with all IFIDs in order.
        #[test]
        fn detect_written_record(stories in vec(nice_story(), 1..3)) {
            let index = IfIndex { stories };
            let xml = write::to_string(&index);
            let d = detect::detect(xml.as_bytes());
            let all: Vec<Ifid> = index
                .stories
                .iter()
                .flat_map(|s| s.identification.ifids.clone())
                .collect();
            prop_assert_eq!(d.kind, FileKind::IFiction);
            prop_assert_eq!(d.source, Source::IFiction);
            prop_assert_eq!(d.ifids, all);
        }

        /// A record embedded in a binary without a marker is found.
        #[test]
        fn detect_embedded_record(
            story in nice_story(),
            before in vec(any::<u8>(), 0..128),
            after in vec(any::<u8>(), 0..128),
        ) {
            let xml = write::to_string(&IfIndex::single(story.clone()));
            let mut bytes = b"\x7fELF".to_vec();
            bytes.extend(&before);
            bytes.extend(xml.bytes());
            bytes.extend(&after);
            prop_assume!(!contains(&bytes, b"UUID://"));
            prop_assume!(!contains(&before, b"<ifindex"));
            let d = detect::detect(&bytes);
            prop_assert_eq!(d.source, Source::Embedded);
            prop_assert_eq!(d.ifids, story.identification.ifids);
        }

        /// A page with an injected meta tag is detected by it.
        #[test]
        fn detect_injected_html((page, _) in html_page(), ifid in ifid()) {
            let out = html::inject(&page, &ifid).unwrap();
            let d = detect::detect(out.as_bytes());
            prop_assert_eq!(d.kind, FileKind::Html);
            prop_assert_eq!(d.source, Source::Html(html::HtmlSource::Meta));
            prop_assert_eq!(d.ifids, vec![ifid]);
        }
    }
}
