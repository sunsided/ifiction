//! Structured records: a `Story` built from arbitrary data.
//!
//! The library types do not derive `Arbitrary`, so a mirror of them does, and converts.
//!
//! Invariants: `lint::check` never panics, on any story; and a story that the model can
//! express in XML writes deterministically and reads back unchanged. A story qualifies
//! when its text is what the reader produces: whitespace runs collapsed to one space,
//! trimmed, no empty values, no characters that XML 1.0 forbids, description paragraphs
//! non-empty, dates that print as `YYYY` or `YYYY-MM-DD`, and a format section only under
//! the format's own name. Others are skipped, not asserted on.

#![no_main]

use arbitrary::Arbitrary;
use ifiction::{
    Auxiliary, Bibliographic, Colophon, Contacts, Cover, CoverFormat, Forgiveness, Format,
    FormatSection, Identification, IfIndex, Ifid, PartialDate, Release, Releases, Story, lint,
    read, write,
};
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
struct AStory {
    ifids: Vec<AIfid>,
    format: AFormat,
    tuid: Option<String>,
    bafn: Option<u32>,
    title: String,
    author: String,
    language: Option<String>,
    headline: Option<String>,
    first_published: Option<ADate>,
    genre: Option<String>,
    group: Option<String>,
    /// Paragraphs; joined with `\n`.
    description: Option<Vec<String>>,
    series: Option<String>,
    series_number: Option<u32>,
    forgiveness: Option<AForgiveness>,
    resources: Vec<(String, String)>,
    contacts: Option<(Option<String>, Option<String>)>,
    cover: Option<(bool, u32, u32, Option<String>)>,
    releases: Option<(Option<ARelease>, Vec<ARelease>)>,
    colophon: Option<(String, Option<String>, ADate)>,
    format_section: Option<(String, Vec<(String, String)>)>,
}

/// An IFID from the allowed alphabet; the length may be out of range.
#[derive(Debug, Arbitrary)]
struct AIfid(Vec<u8>);

#[derive(Debug, Arbitrary)]
enum AFormat {
    Known(u8),
    Other(String),
}

#[derive(Debug, Arbitrary)]
enum ADate {
    Year(u16),
    Date(u16, u8, u8),
}

#[derive(Debug, Arbitrary)]
enum AForgiveness {
    Merciful,
    Polite,
    Tough,
    Nasty,
    Cruel,
}

#[derive(Debug, Arbitrary)]
struct ARelease {
    date: ADate,
    version: Option<u32>,
    compiler: Option<String>,
    compiler_version: Option<String>,
}

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-";
const FORMATS: [&str; 14] = [
    "zcode",
    "glulx",
    "tads2",
    "tads3",
    "hugo",
    "alan",
    "adrift",
    "level9",
    "agt",
    "magscrolls",
    "advsys",
    "quest",
    "html",
    "executable",
];
/// The children of `<story>` that a format section must not shadow.
const STORY_CHILDREN: [&str; 7] = [
    "identification",
    "bibliographic",
    "resources",
    "contacts",
    "cover",
    "releases",
    "colophon",
];

impl AIfid {
    fn to_ifid(&self) -> Option<Ifid> {
        let s: String = self
            .0
            .iter()
            .map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()]))
            .collect();
        Ifid::parse(&s).ok()
    }
}

impl ADate {
    fn to_date(&self) -> PartialDate {
        match *self {
            Self::Year(year) => PartialDate::Year(year),
            Self::Date(year, month, day) => PartialDate::Date { year, month, day },
        }
    }
}

impl AForgiveness {
    fn to_forgiveness(&self) -> Forgiveness {
        match self {
            Self::Merciful => Forgiveness::Merciful,
            Self::Polite => Forgiveness::Polite,
            Self::Tough => Forgiveness::Tough,
            Self::Nasty => Forgiveness::Nasty,
            Self::Cruel => Forgiveness::Cruel,
        }
    }
}

impl ARelease {
    fn to_release(&self) -> Release {
        Release {
            release_date: self.date.to_date(),
            version: self.version,
            compiler: self.compiler.clone(),
            compiler_version: self.compiler_version.clone(),
        }
    }
}

impl AStory {
    /// The story as given, without any normalization.
    fn to_story(&self) -> Story {
        let format = match &self.format {
            AFormat::Known(i) => Format::from(FORMATS[usize::from(*i) % FORMATS.len()]),
            AFormat::Other(s) => Format::Other(s.clone()),
        };
        Story {
            identification: Identification {
                ifids: self.ifids.iter().filter_map(AIfid::to_ifid).collect(),
                format,
                tuid: self.tuid.clone(),
                bafn: self.bafn,
            },
            bibliographic: Bibliographic {
                title: self.title.clone(),
                author: self.author.clone(),
                language: self.language.clone(),
                headline: self.headline.clone(),
                first_published: self.first_published.as_ref().map(ADate::to_date),
                genre: self.genre.clone(),
                group: self.group.clone(),
                description: self.description.as_ref().map(|p| p.join("\n")),
                series: self.series.clone(),
                series_number: self.series_number,
                forgiveness: self.forgiveness.as_ref().map(AForgiveness::to_forgiveness),
            },
            resources: self
                .resources
                .iter()
                .map(|(leafname, description)| Auxiliary {
                    leafname: leafname.clone(),
                    description: description.clone(),
                })
                .collect(),
            contacts: self.contacts.as_ref().map(|(url, email)| Contacts {
                url: url.clone(),
                author_email: email.clone(),
            }),
            cover: self
                .cover
                .as_ref()
                .map(|(png, width, height, description)| Cover {
                    format: if *png {
                        CoverFormat::Png
                    } else {
                        CoverFormat::Jpg
                    },
                    width: *width,
                    height: *height,
                    description: description.clone(),
                }),
            releases: self.releases.as_ref().map(|(attached, history)| Releases {
                attached: attached.as_ref().map(ARelease::to_release),
                history: history.iter().map(ARelease::to_release).collect(),
            }),
            colophon: self
                .colophon
                .as_ref()
                .map(|(generator, version, originated)| Colophon {
                    generator: generator.clone(),
                    generator_version: version.clone(),
                    originated: originated.to_date(),
                }),
            format_section: self
                .format_section
                .as_ref()
                .map(|(name, entries)| FormatSection {
                    name: name.clone(),
                    entries: entries.clone(),
                }),
        }
    }
}

/// Collapses whitespace like the reader does; `None` if nothing is left or XML 1.0 cannot
/// hold a character.
fn text(s: &str) -> Option<String> {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty() && t.chars().all(xml_char)).then_some(t)
}

/// Like [`text`] for an optional value: an empty value is dropped, not a reason to skip.
fn opt(s: Option<&String>) -> Option<Option<String>> {
    match s {
        None => Some(None),
        Some(s) if s.split_whitespace().next().is_none() => Some(None),
        Some(s) => text(s).map(Some),
    }
}

/// The `Char` production of XML 1.0.
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}')
        || c >= '\u{10000}'
}

/// A simple XML element name, without a namespace prefix.
fn xml_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn date(d: PartialDate) -> Option<PartialDate> {
    (d.to_string().parse() == Ok(d)).then_some(d)
}

/// The story as the reader would see it, or `None` if it cannot be expressed in XML.
fn normalize(s: &Story) -> Option<Story> {
    let id = &s.identification;
    if id.ifids.is_empty() {
        return None;
    }
    let format = Format::from(text(id.format.as_str())?);
    let b = &s.bibliographic;
    let description = match &b.description {
        None => None,
        Some(d) => {
            let paragraphs = d
                .split('\n')
                .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>();
            if !paragraphs.iter().flat_map(|p| p.chars()).all(xml_char) {
                return None;
            }
            (!paragraphs.is_empty()).then(|| paragraphs.join("\n"))
        }
    };
    let bibliographic = Bibliographic {
        title: text(&b.title)?,
        author: text(&b.author)?,
        language: opt(b.language.as_ref())?,
        headline: opt(b.headline.as_ref())?,
        first_published: b
            .first_published
            .map(date)
            .map_or(Some(None), |d| d.map(Some))?,
        genre: opt(b.genre.as_ref())?,
        group: opt(b.group.as_ref())?,
        description,
        series: opt(b.series.as_ref())?,
        series_number: b.series_number,
        forgiveness: b.forgiveness,
    };
    let resources = s
        .resources
        .iter()
        .map(|a| {
            Some(Auxiliary {
                leafname: text(&a.leafname)?,
                description: text(&a.description)?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let contacts = match &s.contacts {
        None => None,
        Some(c) => Some(Contacts {
            url: opt(c.url.as_ref())?,
            author_email: opt(c.author_email.as_ref())?,
        }),
    };
    let cover = match &s.cover {
        None => None,
        Some(c) => Some(Cover {
            description: opt(c.description.as_ref())?,
            ..c.clone()
        }),
    };
    let release = |r: &Release| {
        Some(Release {
            release_date: date(r.release_date)?,
            version: r.version,
            compiler: opt(r.compiler.as_ref())?,
            compiler_version: opt(r.compiler_version.as_ref())?,
        })
    };
    let releases = match &s.releases {
        None => None,
        Some(r) => Some(Releases {
            attached: match &r.attached {
                None => None,
                Some(a) => Some(release(a)?),
            },
            history: r.history.iter().map(release).collect::<Option<Vec<_>>>()?,
        }),
    };
    let colophon = match &s.colophon {
        None => None,
        Some(c) => Some(Colophon {
            generator: text(&c.generator)?,
            generator_version: opt(c.generator_version.as_ref())?,
            originated: date(c.originated)?,
        }),
    };
    // The reader finds a format section only by the name of the format.
    let format_section = match &s.format_section {
        None => None,
        Some(f) => {
            let name = format.as_str();
            if !xml_name(name) || STORY_CHILDREN.contains(&name) {
                return None;
            }
            let entries = f
                .entries
                .iter()
                .map(|(k, v)| {
                    if !xml_name(k) {
                        return None;
                    }
                    // Entries may be empty, but keep their text single-line.
                    let v = v.split_whitespace().collect::<Vec<_>>().join(" ");
                    v.chars().all(xml_char).then(|| (k.clone(), v))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(FormatSection {
                name: name.to_owned(),
                entries,
            })
        }
    };
    Some(Story {
        identification: Identification {
            ifids: id.ifids.clone(),
            format,
            tuid: opt(id.tuid.as_ref())?,
            bafn: id.bafn,
        },
        bibliographic,
        resources,
        contacts,
        cover,
        releases,
        colophon,
        format_section,
    })
}

fuzz_target!(|input: AStory| {
    let raw = input.to_story();
    let _ = lint::check(&raw);
    let _ = write::to_string(&IfIndex::single(raw.clone()));

    let Some(story) = normalize(&raw) else {
        return;
    };
    let _ = lint::check(&story);
    let index = IfIndex::single(story);
    let out = write::to_string(&index);
    assert_eq!(
        out,
        write::to_string(&index),
        "writing is not deterministic"
    );
    let again =
        read::from_str(&out).unwrap_or_else(|e| panic!("written record does not read: {e}\n{out}"));
    assert_eq!(again, index, "round trip changed the record\n{out}");
});
