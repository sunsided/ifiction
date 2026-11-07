//! IFDB, the Interactive Fiction Database: fetch a listing, upload a record.
//!
//! API documentation: <https://ifdb.org/api/>. IFDB asks for low-volume use and a
//! descriptive user agent; this module sends one request per command.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// The IFDB server; `IFDB_URL` overrides it (for tests or a mirror).
pub fn base_url() -> String {
    std::env::var("IFDB_URL")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "https://ifdb.org".to_owned())
        .trim_end_matches('/')
        .to_owned()
}

const USER_AGENT: &str = concat!(
    "ifiction-cli/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/sunsided/ifiction)"
);

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

/// What to look up.
pub enum Key<'a> {
    Ifid(&'a str),
    Tuid(&'a str),
}

/// Fetches a listing as iFiction XML or as JSON.
///
/// IFDB's iFiction is a copy of its (wiki-editable) listing, not the author's record, and
/// it adds IFDB's own `<ifdb>` section.
pub fn fetch(key: Key<'_>, json: bool) -> Result<String> {
    let kind = if json { "json" } else { "ifiction" };
    let (param, value) = match key {
        Key::Ifid(i) => ("ifid", i),
        Key::Tuid(t) => ("id", t),
    };
    let url = format!("{}/viewgame?{kind}&{param}={}", base_url(), encode(value));
    let mut resp = agent()
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .context("reading IFDB's answer")?;
    if let Some(err) = viewgame_error(&body) {
        bail!("IFDB: {err}");
    }
    if !status.is_success() {
        bail!("IFDB answered {status}: {}", body.trim());
    }
    Ok(body)
}

/// `<viewgame><errorCode>…</errorCode><errorMessage>…</errorMessage></viewgame>`, or a
/// JSON object with the same fields.
fn viewgame_error(body: &str) -> Option<String> {
    let code = tag_text(body, "errorCode").or_else(|| json_string(body, "errorCode"))?;
    let message = tag_text(body, "errorMessage")
        .or_else(|| json_string(body, "errorMessage"))
        .unwrap_or_default();
    Some(
        format!("{code}: {message}")
            .trim_end_matches(": ")
            .to_owned(),
    )
}

/// The fields of an upload. See <https://ifdb.org/api/putific>.
pub struct Upload<'a> {
    pub username: String,
    pub password: String,
    pub ifiction: String,
    pub cover: Option<(&'a Path, Vec<u8>)>,
    pub links: Option<String>,
    pub replace_links: bool,
    pub image_copyright_status: Option<String>,
    pub image_copyright: Option<String>,
    pub last_version: Option<String>,
}

/// IFDB's answer to an upload.
#[derive(Debug, PartialEq, Eq)]
pub struct Uploaded {
    /// `created`, `edited` or `unchanged`.
    pub action: String,
    pub tuid: Option<String>,
    pub view_url: Option<String>,
}

impl Upload<'_> {
    /// The parts of the form, without the password's value; for `--dry-run`.
    pub fn describe(&self) -> Vec<String> {
        let mut out = vec![
            format!("username: {}", self.username),
            "password: (hidden)".to_owned(),
            format!("ifiction: text/xml, {} bytes", self.ifiction.len()),
        ];
        if let Some((path, bytes)) = &self.cover {
            out.push(format!(
                "coverart: {}, {}, {} bytes",
                path.display(),
                image_type(bytes),
                bytes.len()
            ));
        }
        if let Some(l) = &self.links {
            out.push(format!("links: text/xml, {} bytes", l.len()));
        }
        if self.replace_links {
            out.push("replaceLinks: yes".to_owned());
        }
        if let Some(s) = &self.image_copyright_status {
            out.push(format!("imageCopyrightStatus: {s}"));
        }
        if let Some(s) = &self.image_copyright {
            out.push(format!("imageCopyright: {s}"));
        }
        if let Some(s) = &self.last_version {
            out.push(format!("lastversion: {s}"));
        }
        out
    }

    /// The `multipart/form-data` body and its boundary.
    pub fn form(&self) -> (String, Vec<u8>) {
        let boundary = format!("ifiction-{:016x}", boundary_seed(&self.ifiction));
        let mut body = Vec::new();
        let mut text = |name: &str, value: &str| {
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
                )
                .as_bytes(),
            );
        };
        text("username", &self.username);
        text("password", &self.password);
        if self.replace_links {
            text("replaceLinks", "yes");
        }
        if let Some(s) = &self.image_copyright_status {
            text("imageCopyrightStatus", s);
        }
        if let Some(s) = &self.image_copyright {
            text("imageCopyright", s);
        }
        if let Some(s) = &self.last_version {
            text("lastversion", s);
        }
        let mut file = |name: &str, filename: &str, content_type: &str, bytes: &[u8]| {
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: {content_type}\r\nContent-Transfer-Encoding: binary\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(bytes);
            body.extend_from_slice(b"\r\n");
        };
        file(
            "ifiction",
            "story.iFiction",
            "text/xml",
            self.ifiction.as_bytes(),
        );
        if let Some(links) = &self.links {
            file("links", "links.xml", "text/xml", links.as_bytes());
        }
        if let Some((path, bytes)) = &self.cover {
            let name = path
                .file_name()
                .map_or("cover".into(), |n| n.to_string_lossy().replace('"', ""));
            file("coverart", &name, image_type(bytes), bytes);
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        (boundary, body)
    }
}

/// Sends an upload to `putific`.
pub fn put(upload: &Upload<'_>) -> Result<Uploaded> {
    let url = format!("{}/putific", base_url());
    let (boundary, body) = upload.form();
    let mut resp = agent()
        .post(&url)
        .header("User-Agent", USER_AGENT)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .send(&body[..])
        .with_context(|| format!("POST {url}"))?;
    let status = resp.status();
    let answer = resp
        .body_mut()
        .read_to_string()
        .context("reading IFDB's answer")?;
    parse_put_answer(&answer).with_context(|| format!("IFDB answered {status}"))
}

/// Reads `<putific><ok/><created/>…</putific>` or `<putific><error>…</error></putific>`.
fn parse_put_answer(answer: &str) -> Result<Uploaded> {
    if answer.contains("<ok/>") || answer.contains("<ok />") {
        let action = ["created", "edited", "unchanged"]
            .into_iter()
            .find(|a| answer.contains(&format!("<{a}/>")) || answer.contains(&format!("<{a} />")))
            .unwrap_or("ok")
            .to_owned();
        return Ok(Uploaded {
            action,
            tuid: tag_text(answer, "tuid"),
            view_url: tag_text(answer, "viewUrl"),
        });
    }
    let code = tag_text(answer, "code");
    let message = tag_text(answer, "message");
    if code.is_none() && message.is_none() {
        bail!("unexpected answer: {}", answer.trim());
    }
    let mut text = format!(
        "IFDB rejected the upload: {} {}",
        code.unwrap_or_default(),
        message.unwrap_or_default()
    );
    let mut rest = answer;
    while let Some(start) = rest.find("<detail>") {
        let after = &rest[start + 8..];
        let end = after.find("</detail>").unwrap_or(after.len());
        let detail = &after[..end];
        let field = tag_text(detail, "field").unwrap_or_default();
        let desc = tag_text(detail, "desc")
            .or_else(|| tag_text(detail, "error"))
            .unwrap_or_default();
        text.push_str(&format!("\n  {field}: {desc}"));
        rest = &after[end..];
    }
    bail!("{}", text.trim_end())
}

/// The text of the first `<name>…</name>`, with the XML escapes undone.
fn tag_text(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&format!("</{name}>"))?;
    Some(
        xml[start..end]
            .trim()
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&"),
    )
}

/// The value of `"name": "…"` in a JSON text, without unescaping beyond `\"`.
fn json_string(json: &str, name: &str) -> Option<String> {
    let key = format!("\"{name}\"");
    let after = json[json.find(&key)? + key.len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = after.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.extend(chars.next()),
            c => out.push(c),
        }
    }
    None
}

/// Percent-encodes a query value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The content type IFDB gets for a cover; it sniffs the format itself anyway.
fn image_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else {
        "application/octet-stream"
    }
}

/// A boundary that does not occur in the record (FNV-1a of the record and the time).
fn boundary_seed(text: &str) -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes().chain(nanos.to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewgame_errors() {
        let xml = r#"<viewgame xmlns="http://ifdb.org/api/xmlns"><errorCode>notFound</errorCode><errorMessage>No such game</errorMessage></viewgame>"#;
        assert_eq!(
            viewgame_error(xml).as_deref(),
            Some("notFound: No such game")
        );
        let json = r#"{"errorCode": "badRequest", "errorMessage": "No \"id\""}"#;
        assert_eq!(
            viewgame_error(json).as_deref(),
            Some("badRequest: No \"id\"")
        );
        assert_eq!(viewgame_error("<ifindex></ifindex>"), None);
    }

    #[test]
    fn put_answers() {
        let ok = "<putific xmlns=\"http://ifdb.org/api/xmlns\"><ok/><created/><tuid>abc123</tuid><viewUrl>https://ifdb.org/viewgame?id=abc123</viewUrl></putific>";
        assert_eq!(
            parse_put_answer(ok).unwrap(),
            Uploaded {
                action: "created".into(),
                tuid: Some("abc123".into()),
                view_url: Some("https://ifdb.org/viewgame?id=abc123".into()),
            }
        );
        let err = "<putific><error><code>DataValidation</code><message>Bad data</message><detail><field>language</field><desc>unknown</desc></detail></error></putific>";
        let e = parse_put_answer(err).unwrap_err().to_string();
        assert_eq!(
            e,
            "IFDB rejected the upload: DataValidation Bad data\n  language: unknown"
        );
    }

    #[test]
    fn form_has_all_parts() {
        let up = Upload {
            username: "me@example.org".into(),
            password: "s3cret".into(),
            ifiction: "<ifindex/>".into(),
            cover: Some((Path::new("cover.png"), b"\x89PNGdata".to_vec())),
            links: None,
            replace_links: true,
            image_copyright_status: Some("by permission".into()),
            image_copyright: None,
            last_version: None,
        };
        let (boundary, body) = up.form();
        let body = String::from_utf8_lossy(&body);
        assert!(body.starts_with(&format!("--{boundary}\r\n")));
        assert!(body.ends_with(&format!("--{boundary}--\r\n")));
        for part in [
            "name=\"username\"\r\n\r\nme@example.org\r\n",
            "name=\"replaceLinks\"\r\n\r\nyes\r\n",
            "name=\"imageCopyrightStatus\"\r\n\r\nby permission\r\n",
            "name=\"ifiction\"; filename=\"story.iFiction\"\r\nContent-Type: text/xml\r\n",
            "name=\"coverart\"; filename=\"cover.png\"\r\nContent-Type: image/png\r\n",
        ] {
            assert!(body.contains(part), "{part:?} missing");
        }
        assert!(!up.describe().join("\n").contains("s3cret"));
    }

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("ZCODE-88-840726"), "ZCODE-88-840726");
        assert_eq!(encode("a b&c"), "a%20b%26c");
    }
}
