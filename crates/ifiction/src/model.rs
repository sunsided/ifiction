//! The iFiction record (Treaty of Babel revision 13, section 5).
//!
//! The types follow the XML sections. Text values hold plain text; the writer escapes them.
//! A description holds its paragraphs separated by `\n`; the writer turns them into `<br/>`.

use std::fmt;
use std::str::FromStr;

use crate::Ifid;

/// The namespace of iFiction records.
pub const NAMESPACE: &str = "http://babel.ifarchive.org/protocol/iFiction/";

/// A whole `.iFiction` file: one or more story records.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IfIndex {
    pub stories: Vec<Story>,
}

impl IfIndex {
    /// An index with a single story.
    pub fn single(story: Story) -> Self {
        Self {
            stories: vec![story],
        }
    }
}

/// One `<story>` record.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Story {
    pub identification: Identification,
    pub bibliographic: Bibliographic,
    pub resources: Vec<Auxiliary>,
    pub contacts: Option<Contacts>,
    pub cover: Option<Cover>,
    pub releases: Option<Releases>,
    pub colophon: Option<Colophon>,
    /// The section named after the format, such as `<zcode>` or `<glulx>`.
    pub format_section: Option<FormatSection>,
}

impl Story {
    /// A minimal record: one IFID, a format, a title and an author.
    pub fn new(
        ifid: Ifid,
        format: Format,
        title: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            identification: Identification {
                ifids: vec![ifid],
                format,
                tuid: None,
                bafn: None,
            },
            bibliographic: Bibliographic::new(title, author),
            resources: Vec::new(),
            contacts: None,
            cover: None,
            releases: None,
            colophon: None,
            format_section: None,
        }
    }

    /// The first IFID, which the Treaty treats as the primary one.
    pub fn primary_ifid(&self) -> Option<&Ifid> {
        self.identification.ifids.first()
    }
}

/// `<identification>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Identification {
    /// At least one. The first is the primary IFID.
    pub ifids: Vec<Ifid>,
    pub format: Format,
    /// The IFDB listing ID.
    pub tuid: Option<String>,
    /// The Baf's Guide ID. Not expected in new records.
    pub bafn: Option<u32>,
}

/// `<bibliographic>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bibliographic {
    pub title: String,
    pub author: String,
    /// An ISO 639 code, optionally with an ISO 3166 region: `en`, `en-US`.
    pub language: Option<String>,
    pub headline: Option<String>,
    pub first_published: Option<PartialDate>,
    /// Free text; the Treaty suggests the values in [`GENRES`].
    pub genre: Option<String>,
    pub group: Option<String>,
    /// Paragraphs separated by `\n`.
    pub description: Option<String>,
    pub series: Option<String>,
    pub series_number: Option<u32>,
    pub forgiveness: Option<Forgiveness>,
}

impl Bibliographic {
    /// Only a title and an author.
    pub fn new(title: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            author: author.into(),
            ..Self::default()
        }
    }
}

/// The genres that the Treaty suggests (section 5.6).
pub const GENRES: [&str; 17] = [
    "Children's Fiction",
    "Collegiate Fiction",
    "Comedy",
    "Erotica",
    "Fairy Tale",
    "Fantasy",
    "Fiction",
    "Historical",
    "Horror",
    "Mystery",
    "Non-Fiction",
    "Other",
    "Religious Fiction",
    "Romance",
    "Science Fiction",
    "Surreal",
    "Western",
];

/// `<auxiliary>` in `<resources>`: a file that ships with the story.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Auxiliary {
    pub leafname: String,
    pub description: String,
}

/// `<contacts>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Contacts {
    pub url: Option<String>,
    /// One or more bare addresses, separated by commas.
    pub author_email: Option<String>,
}

/// `<cover>`: describes the cover image that ships with the story.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cover {
    pub format: CoverFormat,
    pub width: u32,
    pub height: u32,
    pub description: Option<String>,
}

/// The two cover formats the Treaty allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum CoverFormat {
    Jpg,
    Png,
}

impl CoverFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Png => "png",
        }
    }
}

/// `<releases>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Releases {
    /// The release of the file this record is attached to.
    pub attached: Option<Release>,
    pub history: Vec<Release>,
}

/// `<release>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Release {
    pub release_date: PartialDate,
    /// The release number, a non-negative integer.
    pub version: Option<u32>,
    /// The major version only: "Inform 6", not "Inform 6.12".
    pub compiler: Option<String>,
    pub compiler_version: Option<String>,
}

/// `<colophon>`: who made the record, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Colophon {
    pub generator: String,
    pub generator_version: Option<String>,
    pub originated: PartialDate,
}

/// A format section such as `<zcode>`, kept as flat name and value pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormatSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// `<format>`: the formats of section 5.5.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub enum Format {
    Zcode,
    Glulx,
    Tads2,
    Tads3,
    Hugo,
    Alan,
    Adrift,
    Level9,
    Agt,
    Magscrolls,
    Advsys,
    /// Quest 5.1+ games (Treaty revision 13).
    Quest,
    /// A web page; also the format of a WASM game that runs in one.
    Html,
    /// A native program.
    Executable,
    /// A format that is not in the Treaty yet.
    Other(String),
}

impl Format {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Zcode => "zcode",
            Self::Glulx => "glulx",
            Self::Tads2 => "tads2",
            Self::Tads3 => "tads3",
            Self::Hugo => "hugo",
            Self::Alan => "alan",
            Self::Adrift => "adrift",
            Self::Level9 => "level9",
            Self::Agt => "agt",
            Self::Magscrolls => "magscrolls",
            Self::Advsys => "advsys",
            Self::Quest => "quest",
            Self::Html => "html",
            Self::Executable => "executable",
            Self::Other(s) => s,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Format {
    fn from(s: &str) -> Self {
        match s {
            "zcode" => Self::Zcode,
            "glulx" => Self::Glulx,
            "tads2" => Self::Tads2,
            "tads3" => Self::Tads3,
            "hugo" => Self::Hugo,
            "alan" => Self::Alan,
            "adrift" => Self::Adrift,
            "level9" => Self::Level9,
            "agt" => Self::Agt,
            "magscrolls" => Self::Magscrolls,
            "advsys" => Self::Advsys,
            "quest" => Self::Quest,
            "html" => Self::Html,
            "executable" => Self::Executable,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl From<String> for Format {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl From<Format> for String {
    fn from(f: Format) -> Self {
        f.as_str().to_owned()
    }
}

/// `<forgiveness>`: the Zarfian cruelty scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Forgiveness {
    Merciful,
    Polite,
    Tough,
    Nasty,
    Cruel,
}

impl Forgiveness {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merciful => "Merciful",
            Self::Polite => "Polite",
            Self::Tough => "Tough",
            Self::Nasty => "Nasty",
            Self::Cruel => "Cruel",
        }
    }
}

/// Not one of the five forgiveness values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgivenessError(pub String);

impl fmt::Display for ForgivenessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "forgiveness is one of Merciful, Polite, Tough, Nasty, Cruel, not {:?}",
            self.0
        )
    }
}

impl std::error::Error for ForgivenessError {}

impl FromStr for Forgiveness {
    type Err = ForgivenessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Merciful" => Self::Merciful,
            "Polite" => Self::Polite,
            "Tough" => Self::Tough,
            "Nasty" => Self::Nasty,
            "Cruel" => Self::Cruel,
            _ => return Err(ForgivenessError(s.to_owned())),
        })
    }
}

/// A date as the Treaty writes it: `YYYY` or `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub enum PartialDate {
    Year(u16),
    Date { year: u16, month: u8, day: u8 },
}

/// Not a date of the form `YYYY` or `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateError(pub String);

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a date is YYYY or YYYY-MM-DD, not {:?}", self.0)
    }
}

impl std::error::Error for DateError {}

impl PartialDate {
    /// The calendar day of a Unix timestamp, in UTC.
    pub fn from_unix_seconds(secs: i64) -> Self {
        // Howard Hinnant's civil_from_days.
        let z = secs.div_euclid(86_400) + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self::Date {
            year: year.clamp(0, 9999) as u16,
            month,
            day,
        }
    }
}

impl fmt::Display for PartialDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Year(y) => write!(f, "{y:04}"),
            Self::Date { year, month, day } => write!(f, "{year:04}-{month:02}-{day:02}"),
        }
    }
}

impl FromStr for PartialDate {
    type Err = DateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || DateError(s.to_owned());
        let digits = |p: &str, n: usize| p.len() == n && p.bytes().all(|b| b.is_ascii_digit());
        let parts: Vec<&str> = s.split('-').collect();
        match parts.as_slice() {
            [y] if digits(y, 4) => Ok(Self::Year(y.parse().map_err(|_| err())?)),
            [y, m, d] if digits(y, 4) && digits(m, 2) && digits(d, 2) => {
                let year: u16 = y.parse().map_err(|_| err())?;
                let month: u8 = m.parse().map_err(|_| err())?;
                let day: u8 = d.parse().map_err(|_| err())?;
                if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
                    return Err(err());
                }
                Ok(Self::Date { year, month, day })
            }
            _ => Err(err()),
        }
    }
}

impl TryFrom<String> for PartialDate {
    type Error = DateError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<PartialDate> for String {
    fn from(d: PartialDate) -> Self {
        d.to_string()
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!("2006".parse(), Ok(PartialDate::Year(2006)));
        assert_eq!(
            "2024-02-29".parse(),
            Ok(PartialDate::Date {
                year: 2024,
                month: 2,
                day: 29
            })
        );
        assert!("2023-02-29".parse::<PartialDate>().is_err());
        assert!("2006-04".parse::<PartialDate>().is_err());
        assert!("06".parse::<PartialDate>().is_err());
        assert_eq!(PartialDate::Year(987).to_string(), "0987");
    }

    #[test]
    fn unix_seconds() {
        assert_eq!(PartialDate::from_unix_seconds(0).to_string(), "1970-01-01");
        assert_eq!(
            PartialDate::from_unix_seconds(1_709_164_800).to_string(),
            "2024-02-29"
        );
        assert_eq!(
            PartialDate::from_unix_seconds(1_794_009_600).to_string(),
            "2026-11-07"
        );
    }

    #[test]
    fn formats_round_trip() {
        for f in ["zcode", "html", "executable", "tads3", "brand-new"] {
            assert_eq!(Format::from(f).as_str(), f);
        }
    }
}
