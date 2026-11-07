//! IFIDs: the identifiers of the Treaty of Babel (section 2.2).

use std::fmt;
use std::str::FromStr;

/// The smallest length of an IFID.
pub const MIN_LEN: usize = 8;
/// The largest length of an IFID.
pub const MAX_LEN: usize = 63;

/// An IFID: 8 to 63 characters, each a digit, a capital letter or a hyphen.
///
/// An IFID names a work, not a file. Rebuilds and bug-fix releases keep it. A port to
/// another format (for example from a native executable to a web page) gets a new one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct Ifid(String);

/// Why a string is not an IFID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IfidError {
    Length(usize),
    Char(char),
}

impl fmt::Display for IfidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(n) => write!(f, "an IFID has 8 to 63 characters, this one has {n}"),
            Self::Char(c) => write!(
                f,
                "an IFID has only digits, capital letters and hyphens, this one has {c:?}"
            ),
        }
    }
}

impl std::error::Error for IfidError {}

impl Ifid {
    /// Parses an IFID strictly: lowercase letters are an error.
    pub fn parse(s: &str) -> Result<Self, IfidError> {
        let len = s.chars().count();
        if !(MIN_LEN..=MAX_LEN).contains(&len) {
            return Err(IfidError::Length(len));
        }
        if let Some(c) = s
            .chars()
            .find(|c| !(c.is_ascii_digit() || c.is_ascii_uppercase() || *c == '-'))
        {
            return Err(IfidError::Char(c));
        }
        Ok(Self(s.to_owned()))
    }

    /// Parses an IFID after it trims whitespace and capitalizes letters.
    ///
    /// The Treaty asks for capitals, but UUID tools often print lowercase.
    pub fn parse_lenient(s: &str) -> Result<Self, IfidError> {
        Self::parse(&s.trim().to_ascii_uppercase())
    }

    /// A new IFID from a random (version 4) UUID, in capitals, as the Treaty recommends.
    #[cfg(feature = "generate")]
    pub fn new_v4() -> Self {
        let mut buf = uuid::Uuid::encode_buffer();
        let s = uuid::Uuid::new_v4().hyphenated().encode_upper(&mut buf);
        Self((*s).to_owned())
    }

    /// The IFID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The marker for story files: `UUID://<IFID>//`.
    pub fn marker(&self) -> String {
        format!("UUID://{}//", self.0)
    }
}

impl fmt::Display for Ifid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Ifid {
    type Err = IfidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for Ifid {
    type Error = IfidError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<Ifid> for String {
    fn from(ifid: Ifid) -> Self {
        ifid.0
    }
}

impl AsRef<str> for Ifid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_treaty_examples() {
        for s in [
            "1974A053-7DB0-4103-93A1-767C1382C0B7",
            "ZCODE-11-860509",
            "ZCODE-8-040205-6630",
            "MAGNETIC-1",
            "TADS2-C15B8633FF25B25DB1E61DE870D19D68",
        ] {
            assert!(Ifid::parse(s).is_ok(), "{s}");
        }
    }

    #[test]
    fn rejects_bad_ifids() {
        assert_eq!(Ifid::parse("SHORT"), Err(IfidError::Length(5)));
        assert_eq!(Ifid::parse(&"A".repeat(64)), Err(IfidError::Length(64)));
        assert_eq!(Ifid::parse("1974a053-7DB0"), Err(IfidError::Char('a')));
        assert_eq!(Ifid::parse("ABCD EFGH"), Err(IfidError::Char(' ')));
    }

    #[test]
    fn lenient_capitalizes() {
        let ifid = Ifid::parse_lenient(" 1974a053-7db0-4103-93a1-767c1382c0b7\n").unwrap();
        assert_eq!(ifid.as_str(), "1974A053-7DB0-4103-93A1-767C1382C0B7");
    }

    #[test]
    fn marker() {
        let ifid = Ifid::parse("1974A053-7DB0-4103-93A1-767C1382C0B7").unwrap();
        assert_eq!(
            ifid.marker(),
            "UUID://1974A053-7DB0-4103-93A1-767C1382C0B7//"
        );
    }

    #[cfg(feature = "generate")]
    #[test]
    fn generated_ifids_are_valid_and_distinct() {
        let a = Ifid::new_v4();
        let b = Ifid::new_v4();
        assert_ne!(a, b);
        assert!(Ifid::parse(a.as_str()).is_ok());
        assert_eq!(a.as_str().len(), 36);
    }
}
