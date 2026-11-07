//! IFIDs in HTML story files (Treaty sections 2.2.2 and 2.2.3.10).
//!
//! The preferred form is a `<meta property="ifiction:ifid">` tag in `<head>`. babel looks
//! only at the `property` attribute; the RDFa `prefix` is recommended, so [`meta_tag`]
//! writes it.

use crate::Ifid;
use crate::model::NAMESPACE;
use crate::scan;

/// Where an IFID in an HTML file came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlSource {
    /// `<meta property="ifiction:ifid">`.
    Meta,
    /// A `UUID://…//` marker in a literal or a comment.
    Marker,
    /// The `ifid` attribute of Twine's `<tw-storydata>`.
    Twine,
}

/// Why the IFID could not be injected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlError {
    NoHead,
}

impl std::fmt::Display for HtmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoHead => f.write_str("the HTML has no <head> element"),
        }
    }
}

impl std::error::Error for HtmlError {}

/// The meta tag for an IFID, with the RDFa prefix.
pub fn meta_tag(ifid: &Ifid) -> String {
    format!(r#"<meta prefix="ifiction: {NAMESPACE}" property="ifiction:ifid" content="{ifid}">"#)
}

/// Puts the IFID meta tag into `<head>`, or replaces the one that is there.
///
/// Injecting the same IFID twice gives the same document.
pub fn inject(html: &str, ifid: &Ifid) -> Result<String, HtmlError> {
    let tag = meta_tag(ifid);
    if let Some((start, end)) = tags(html, "meta").find(|&(s, e)| is_ifid_meta(&html[s..e])) {
        return Ok(format!("{}{tag}{}", &html[..start], &html[end..]));
    }
    let (head_start, mut head_end) = tags(html, "head").next().ok_or(HtmlError::NoHead)?;
    // Keep `<meta charset>` first, as browsers want it early.
    let next = html[head_end..].trim_start();
    if let Some((s, e)) = tags(next, "meta").next()
        && s == 0
        && attr(&next[..e], "charset").is_some()
    {
        head_end = html.len() - next.len() + e;
    }
    // Indent like the first line inside <head>, or two spaces more than <head> itself.
    let rest = &html[head_end..];
    let indent = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))
        .map(|r| {
            r.chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect::<String>()
        })
        .unwrap_or_else(|| {
            let line_start = html[..head_start].rfind('\n').map_or(0, |i| i + 1);
            let outer: String = html[line_start..head_start]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            format!("{outer}  ")
        });
    Ok(format!(
        "{}\n{indent}{tag}{}",
        &html[..head_end],
        &html[head_end..]
    ))
}

/// Finds the IFID of an HTML story file, in the order babel uses.
///
/// Returns `None` if there is none; then the Treaty's IFID is `HTML-` and the MD5 of the
/// file (see [`crate::detect`]).
pub fn extract_ifid(html: &str) -> Option<(Ifid, HtmlSource)> {
    for (s, e) in tags(html, "meta") {
        let tag = &html[s..e];
        if is_ifid_meta(tag)
            && let Some(ifid) = attr(tag, "content").and_then(|c| Ifid::parse_lenient(&c).ok())
        {
            return Some((ifid, HtmlSource::Meta));
        }
    }
    if let Some(ifid) = scan::find_markers(html.as_bytes()).into_iter().next() {
        return Some((ifid, HtmlSource::Marker));
    }
    for (s, e) in tags(html, "tw-storydata") {
        if let Some(ifid) = attr(&html[s..e], "ifid").and_then(|c| Ifid::parse_lenient(&c).ok()) {
            return Some((ifid, HtmlSource::Twine));
        }
    }
    None
}

fn is_ifid_meta(tag: &str) -> bool {
    attr(tag, "property").is_some_and(|p| p.eq_ignore_ascii_case("ifiction:ifid"))
}

/// The byte ranges of all start tags `<name …>`, ASCII case-insensitive.
fn tags<'a>(html: &'a str, name: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
    let lower = html.to_ascii_lowercase();
    let needle = format!("<{name}");
    let mut from = 0;
    std::iter::from_fn(move || {
        loop {
            let start = from + lower[from..].find(&needle)?;
            let after = start + needle.len();
            from = after;
            let next = lower[after..].chars().next()?;
            if !(next.is_whitespace() || next == '>' || next == '/') {
                continue;
            }
            let end = after + tag_end(&html[after..])?;
            from = end;
            return Some((start, end));
        }
    })
}

/// The end of a tag: just past the next `>`, or just before the next `<`.
///
/// Quotes are not special, as in babel's own scanner: a tag is what babel reads as one, and
/// malformed markup (a stray quote, an unclosed value) cannot swallow the rest of the
/// document. [`inject`]'s tag has neither `<` nor `>` in it, so it is always found again.
/// The cost: a `>` inside an attribute value ends the tag early, for babel as well.
fn tag_end(s: &str) -> Option<usize> {
    let i = s.find(['<', '>'])?;
    Some(if s.as_bytes()[i] == b'>' { i + 1 } else { i })
}

/// The value of an attribute in a start tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let inner = tag
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end_matches('/');
    let mut rest = inner.split_once(|c: char| c.is_whitespace())?.1;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        let key_end = rest
            .find(|c: char| c == '=' || c.is_whitespace())
            .unwrap_or(rest.len());
        let key = &rest[..key_end];
        rest = rest[key_end..].trim_start();
        let value = if let Some(r) = rest.strip_prefix('=') {
            let r = r.trim_start();
            let (value, tail) = match r.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let close = r[1..].find(q).map_or(r.len(), |i| i + 1);
                    (&r[1..close], r.get(close + 1..).unwrap_or(""))
                }
                _ => {
                    let end = r.find(char::is_whitespace).unwrap_or(r.len());
                    (&r[..end], &r[end..])
                }
            };
            rest = tail;
            Some(value)
        } else {
            None
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(value.unwrap_or("").to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IFID: &str = "448E73DF-2D2F-47E7-A494-A46B40D4CFB3";

    fn ifid() -> Ifid {
        Ifid::parse(IFID).unwrap()
    }

    #[test]
    fn injects_into_head_with_indent() {
        let html = "<!doctype html>\n<html>\n  <head>\n    <title>X</title>\n  </head>\n</html>\n";
        let out = inject(html, &ifid()).unwrap();
        assert!(out.contains(&format!("  <head>\n    {}\n    <title>", meta_tag(&ifid()))));
        assert_eq!(inject(&out, &ifid()).unwrap(), out);
        assert_eq!(extract_ifid(&out), Some((ifid(), HtmlSource::Meta)));
    }

    #[test]
    fn replaces_existing_meta() {
        let html =
            r#"<html><HEAD><Meta Property='ifiction:ifid' content='OLD-IFID-1234'></HEAD></html>"#;
        let out = inject(html, &ifid()).unwrap();
        assert!(!out.contains("OLD-IFID"));
        assert_eq!(out.matches("ifiction:ifid").count(), 1);
    }

    #[test]
    fn keeps_charset_first() {
        let html = "<html>\n  <head>\n    <meta charset=\"utf-8\">\n    <title>X</title>\n  </head>\n</html>\n";
        let out = inject(html, &ifid()).unwrap();
        assert!(out.contains(&format!(
            "<meta charset=\"utf-8\">\n    {}\n    <title>",
            meta_tag(&ifid())
        )));
        assert_eq!(inject(&out, &ifid()).unwrap(), out);
    }

    #[test]
    fn stray_quote_does_not_hide_the_tag() {
        // Found by a property test: the quote used to run to the end of the document.
        let once = inject("<meta \"<head>", &ifid()).unwrap();
        assert_eq!(inject(&once, &ifid()).unwrap(), once);
        assert_eq!(extract_ifid(&once), Some((ifid(), HtmlSource::Meta)));
    }

    #[test]
    fn no_head() {
        assert_eq!(inject("<p>hi</p>", &ifid()), Err(HtmlError::NoHead));
    }

    #[test]
    fn does_not_match_header_for_head() {
        let html = "<header></header><head></head>";
        let out = inject(html, &ifid()).unwrap();
        assert!(out.starts_with("<header></header><head>\n"));
    }

    #[test]
    fn falls_back_to_marker_and_twine() {
        let marker = format!("<script>// UUID://{IFID}//\n</script>");
        assert_eq!(extract_ifid(&marker), Some((ifid(), HtmlSource::Marker)));
        let twine = format!(r#"<tw-storydata name="x" ifid="{IFID}" format="Harlowe">"#);
        assert_eq!(extract_ifid(&twine), Some((ifid(), HtmlSource::Twine)));
        assert_eq!(extract_ifid("<html></html>"), None);
    }

    #[test]
    fn gt_in_attribute_ends_the_tag_like_babel() {
        // babel reads `<meta name="a>` as the tag, so it finds no IFID here; neither do we.
        let html = format!(r#"<meta name="a>b" property="ifiction:ifid" content="{IFID}">"#);
        assert_eq!(extract_ifid(&html), None);
    }
}
