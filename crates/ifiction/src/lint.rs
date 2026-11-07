//! Checks a record against the rules (errors) and guidelines (warnings) of the Treaty.
//!
//! Errors are what `babel -verify` rejects; warnings are what `babel -lint` adds.

use std::fmt;

use crate::model::{Format, GENRES, IfIndex, Story};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// The element, such as `bibliographic/title`.
    pub path: String,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{level}: {}: {}", self.path, self.message)
    }
}

/// True if any diagnostic is an error.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Checks every story of an index.
pub fn check_index(index: &IfIndex) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if index.stories.is_empty() {
        out.push(Diagnostic {
            severity: Severity::Error,
            path: "ifindex".into(),
            message: "has no <story>".into(),
        });
    }
    for (i, s) in index.stories.iter().enumerate() {
        for mut d in check(s) {
            if index.stories.len() > 1 {
                d.path = format!("story[{}]/{}", i + 1, d.path);
            }
            out.push(d);
        }
    }
    out
}

/// Checks one story.
pub fn check(story: &Story) -> Vec<Diagnostic> {
    let mut l = Lint::default();
    let id = &story.identification;
    if id.ifids.is_empty() {
        l.error("identification/ifid", "at least one IFID is required");
    }
    if let Format::Other(f) = &id.format {
        l.warn(
            "identification/format",
            format!("{f:?} is not a format of the Treaty"),
        );
    }
    if let Some(t) = &id.tuid
        && !t.chars().all(|c| c.is_ascii_alphanumeric())
    {
        l.error("identification/tuid", "has only letters and digits");
    }

    let b = &story.bibliographic;
    l.line("bibliographic/title", Some(&b.title), true);
    l.line("bibliographic/author", Some(&b.author), true);
    for (path, v) in [
        ("bibliographic/headline", &b.headline),
        ("bibliographic/genre", &b.genre),
        ("bibliographic/group", &b.group),
        ("bibliographic/series", &b.series),
    ] {
        l.line(path, v.as_deref(), false);
    }
    l.shouting("bibliographic/title", &b.title);
    l.shouting("bibliographic/author", &b.author);
    if b.author.to_lowercase().starts_with("by ") {
        l.warn("bibliographic/author", "names only: drop the \"by\"");
    }
    if let Some(lang) = &b.language
        && !valid_language(lang)
    {
        l.error(
            "bibliographic/language",
            format!("{lang:?} is not an ISO 639 code with an optional region, like en or en-US"),
        );
    }
    if let Some(g) = &b.genre
        && !GENRES.contains(&g.as_str())
    {
        l.warn(
            "bibliographic/genre",
            format!("{g:?} is not one of the genres the Treaty suggests"),
        );
    }
    if let Some(d) = &b.description {
        if d.trim().is_empty() {
            l.error("bibliographic/description", "must not be empty");
        } else if d.contains('<') || d.contains('>') {
            l.warn(
                "bibliographic/description",
                "contains < or >; markup other than paragraph breaks is not allowed",
            );
        }
        if d.chars().count() > 2400 {
            l.warn(
                "bibliographic/description",
                "is longer than 2400 characters; tools may truncate it",
            );
        }
    }
    if b.series_number.is_some() && b.series.is_none() {
        l.error("bibliographic/seriesnumber", "needs <series>");
    }

    for (i, a) in story.resources.iter().enumerate() {
        let path = format!("resources/auxiliary[{}]/leafname", i + 1);
        if a.leafname.contains(['/', '\\', ':']) {
            l.error(&path, "is a file name without a path");
        }
        if a.leafname.matches('.').count() > 1 {
            l.error(&path, "has at most one full stop, before the extension");
        }
        l.line(
            &format!("resources/auxiliary[{}]/description", i + 1),
            Some(&a.description),
            true,
        );
    }

    if let Some(c) = &story.contacts {
        if let Some(url) = &c.url
            && !(url.starts_with("http://") || url.starts_with("https://"))
        {
            l.error("contacts/url", "must be an absolute http URL");
        }
        if let Some(mail) = &c.author_email
            && mail
                .split(',')
                .any(|m| !m.trim().contains('@') || m.contains('<'))
        {
            l.error(
                "contacts/authoremail",
                "is bare addresses separated by commas, without names",
            );
        }
    }

    if let Some(c) = &story.cover {
        let (w, h) = (c.width, c.height);
        if w < 120 || h < 120 {
            l.error(
                "cover",
                format!("{w}x{h}: each side must be at least 120 pixels"),
            );
        }
        // In u64: 2 * w overflows u32 from 2^31.
        if u64::from(w) > 2 * u64::from(h) || u64::from(h) > 2 * u64::from(w) {
            l.error(
                "cover",
                format!("{w}x{h}: the aspect ratio must be between 1:2 and 2:1"),
            );
        }
        if w > 1200 || h > 1200 {
            l.warn(
                "cover",
                format!("{w}x{h}: sides should not exceed 1200 pixels"),
            );
        }
        if c.description.is_none() {
            l.warn("cover/description", "a description is recommended");
        }
    }

    if let Some(r) = &story.releases {
        for (i, rel) in r.attached.iter().chain(&r.history).enumerate() {
            if rel.compiler_version.is_some() && rel.compiler.is_none() {
                l.error(
                    &format!("releases/release[{}]/compilerversion", i + 1),
                    "needs <compiler>",
                );
            }
        }
    }

    if let Some(c) = &story.colophon {
        l.line("colophon/generator", Some(&c.generator), true);
    }

    if let Some(f) = &story.format_section
        && f.name != id.format.as_str()
    {
        l.error(
            &f.name,
            format!("the format section must match the format {}", id.format),
        );
    }
    l.out
}

#[derive(Default)]
struct Lint {
    out: Vec<Diagnostic>,
}

impl Lint {
    fn error(&mut self, path: &str, message: impl Into<String>) {
        self.push(Severity::Error, path, message.into());
    }

    fn warn(&mut self, path: &str, message: impl Into<String>) {
        self.push(Severity::Warning, path, message.into());
    }

    fn push(&mut self, severity: Severity, path: &str, message: String) {
        self.out.push(Diagnostic {
            severity,
            path: path.into(),
            message,
        });
    }

    /// The rules for single-line values (section 5.6).
    fn line(&mut self, path: &str, value: Option<&str>, required: bool) {
        let Some(v) = value else {
            if required {
                self.error(path, "is required");
            }
            return;
        };
        if v.is_empty() {
            self.error(path, "must not be empty");
            return;
        }
        if v.trim() != v {
            self.error(path, "must not start or end with whitespace");
        }
        if v.chars().any(|c| c.is_whitespace() && c != ' ') {
            self.error(path, "must not contain tabs, line breaks or special spaces");
        }
        if v.contains("  ") {
            self.error(path, "must not contain runs of spaces");
        }
        if v.contains('<') && v.contains('>') {
            self.warn(path, "looks like it contains markup, which is not allowed");
        }
        if v.chars().count() > 240 {
            self.warn(path, "is longer than 240 characters; tools may truncate it");
        }
    }

    fn shouting(&mut self, path: &str, v: &str) {
        let letters: Vec<char> = v.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() > 3 && letters.iter().all(|c| c.is_uppercase()) {
            self.warn(path, "should not be in capitals");
        }
    }
}

fn valid_language(s: &str) -> bool {
    let mut parts = s.split('-');
    let lang = parts.next().unwrap_or_default();
    let lang_ok = (2..=3).contains(&lang.len()) && lang.bytes().all(|b| b.is_ascii_lowercase());
    let region_ok = match parts.next() {
        None => true,
        Some(r) => r.len() == 2 && r.bytes().all(|b| b.is_ascii_uppercase()),
    };
    lang_ok && region_ok && parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages() {
        assert!(valid_language("en"));
        assert!(valid_language("en-US"));
        assert!(valid_language("deu"));
        assert!(!valid_language("EN"));
        assert!(!valid_language("en_US"));
        assert!(!valid_language("english"));
    }
}
