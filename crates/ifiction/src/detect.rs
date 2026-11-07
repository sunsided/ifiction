//! Finds the IFIDs of any file, the way babel does, plus the embedded forms of this crate.
//!
//! Order: an iFiction record, then HTML (meta tag, marker, Twine), then `UUID://…//` markers
//! and embedded records in binaries, and last the MD5 fallback of the Treaty
//! (`ELF-<MD5>`, `MZ-<MD5>`, `HTML-<MD5>`, …; a bare MD5 for unknown files).
//!
//! This follows the Treaty of Babel revision 13 and the reference tool `babel` 0.75: a
//! marker counts only in capitals; files of unknown format are treated as executables
//! (marker, then a bare MD5); and the prefix of the MD5 fallback comes from babel's exact
//! magic numbers (see [`babel_prefix`]). Embedded records without a marker are an extra of
//! this crate.

use md5::{Digest, Md5};

use crate::Ifid;
use crate::html::{self, HtmlSource};
use crate::scan;

/// What kind of file the bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    IFiction,
    Html,
    /// Linux and other ELF systems.
    Elf,
    /// MS-DOS, Windows, OS/2.
    Mz,
    /// macOS (also fat binaries).
    MachO,
    /// Java class files.
    Java,
    /// AmigaOS hunk files.
    Amiga,
    /// Unix shell scripts (`#!`).
    Script,
    /// WebAssembly modules; not a format of the Treaty.
    Wasm,
    Unknown,
}

impl FileKind {
    /// Sniffs the kind from the first bytes, and from an HTML or XML start.
    pub fn sniff(bytes: &[u8]) -> Self {
        let head = &bytes[..bytes.len().min(1024)];
        let text = String::from_utf8_lossy(head.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(head))
            .trim_start()
            .to_ascii_lowercase();
        let u32_at = |i: usize| {
            bytes
                .get(i..i + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        };
        match bytes {
            [0x7f, b'E', b'L', b'F', ..] => Self::Elf,
            [b'M', b'Z', ..] => Self::Mz,
            [0xfe, 0xed, 0xfa, 0xce | 0xcf, ..] | [0xce | 0xcf, 0xfa, 0xed, 0xfe, ..] => {
                Self::MachO
            }
            // Fat Mach-O and Java share the magic; Java's next word is its version (45+).
            [0xca, 0xfe, 0xba, 0xbe, ..] => match u32_at(4) {
                Some(n) if n < 45 => Self::MachO,
                _ => Self::Java,
            },
            [0x00, 0x00, 0x03, 0xe7 | 0xf3, ..] => Self::Amiga,
            [b'#', b'!', ..] => Self::Script,
            [0x00, b'a', b's', b'm', ..] => Self::Wasm,
            _ if text.starts_with("<?xml") && text.contains("<ifindex") => Self::IFiction,
            _ if text.starts_with("<!doctype html")
                || text.starts_with("<html")
                || text.contains("<head") =>
            {
                Self::Html
            }
            _ => Self::Unknown,
        }
    }
}

/// The prefix of the MD5 fallback IFID for executables, exactly as `babel` derives it
/// (Treaty section 2.2.6.13; `executable.c` of babel 0.75).
///
/// babel knows fewer magic numbers than [`FileKind::sniff`]: only the 32-bit big-endian
/// Mach-O magic `FEEDFACE` counts as `MACHO` (64-bit macOS binaries get a bare MD5), and
/// `CAFEBABE` is always `JAVA`, also for fat Mach-O binaries.
pub fn babel_prefix(bytes: &[u8]) -> &'static str {
    const TABLE: [(&[u8], &str); 7] = [
        (b"MZ", "MZ-"),
        (b"\x7fELF", "ELF-"),
        (b"\xca\xfe\xba\xbe", "JAVA-"),
        (b"\x00\x00\x03\xe7", "AMIGA-"),
        (b"#!", "SCRIPT-"),
        (b"\xfe\xed\xfa\xce", "MACHO-"),
        (b"APPL", "MAC-"),
    ];
    TABLE
        .iter()
        .find(|(magic, _)| bytes.starts_with(magic))
        .map_or("", |(_, prefix)| prefix)
}

/// Where the IFIDs came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The `<ifid>` elements of an iFiction file.
    IFiction,
    Html(HtmlSource),
    /// `UUID://…//` markers in the bytes.
    Marker,
    /// An iFiction record embedded in the bytes, without a marker.
    Embedded,
    /// The MD5 fallback of the Treaty.
    Fallback,
}

/// The result of [`detect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub kind: FileKind,
    pub source: Source,
    /// At least one; the first is the primary IFID.
    pub ifids: Vec<Ifid>,
}

/// Finds the IFIDs of a file.
pub fn detect(bytes: &[u8]) -> Detected {
    let kind = FileKind::sniff(bytes);
    let found = |source, ifids| Detected {
        kind,
        source,
        ifids,
    };
    if kind == FileKind::IFiction
        && let Ok(xml) = std::str::from_utf8(bytes)
        && let Ok(ifids) = crate::read::ifids(xml)
        && !ifids.is_empty()
    {
        return found(Source::IFiction, ifids);
    }
    if kind == FileKind::Html {
        let text = String::from_utf8_lossy(bytes);
        if let Some((ifid, src)) = html::extract_ifid(&text) {
            return found(Source::Html(src), vec![ifid]);
        }
    } else {
        let markers = scan::find_markers(bytes);
        if !markers.is_empty() {
            return found(Source::Marker, markers);
        }
        if let Some((_, index)) = scan::find_ifiction(bytes).into_iter().next() {
            let ifids: Vec<Ifid> = index
                .stories
                .iter()
                .flat_map(|s| s.identification.ifids.iter().cloned())
                .collect();
            if !ifids.is_empty() {
                return found(Source::Embedded, ifids);
            }
        }
    }
    found(Source::Fallback, vec![fallback_ifid(bytes, kind)])
}

/// The MD5 fallback IFID of the Treaty: `HTML-` for web pages, babel's executable prefix
/// ([`babel_prefix`]) for anything else, then the MD5 in capitals.
pub fn fallback_ifid(bytes: &[u8], kind: FileKind) -> Ifid {
    let digest = Md5::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02X}")).collect();
    let prefix = match kind {
        FileKind::Html => "HTML-",
        FileKind::IFiction => "",
        _ => babel_prefix(bytes),
    };
    Ifid::parse(&format!("{prefix}{hex}")).expect("MD5 IFIDs are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs() {
        assert_eq!(FileKind::sniff(b"\x7fELF\x02"), FileKind::Elf);
        assert_eq!(FileKind::sniff(b"MZ\x90\0"), FileKind::Mz);
        assert_eq!(FileKind::sniff(b"\xcf\xfa\xed\xfe"), FileKind::MachO);
        assert_eq!(
            FileKind::sniff(b"\xca\xfe\xba\xbe\0\0\0\x02"),
            FileKind::MachO
        );
        assert_eq!(
            FileKind::sniff(b"\xca\xfe\xba\xbe\0\0\0\x34"),
            FileKind::Java
        );
        assert_eq!(FileKind::sniff(b"\0asm\x01\0\0\0"), FileKind::Wasm);
        assert_eq!(FileKind::sniff(b"  <!DOCTYPE html><html>"), FileKind::Html);
        assert_eq!(
            FileKind::sniff(b"\xEF\xBB\xBF<?xml version=\"1.0\"?><ifindex>"),
            FileKind::IFiction
        );
        assert_eq!(FileKind::sniff(b"hello"), FileKind::Unknown);
    }

    #[test]
    fn fallback() {
        // MD5("") = D41D8CD98F00B204E9800998ECF8427E.
        assert_eq!(
            fallback_ifid(b"", FileKind::Unknown).as_str(),
            "D41D8CD98F00B204E9800998ECF8427E"
        );
        assert_eq!(babel_prefix(b"\xcf\xfa\xed\xfe"), "");
        assert_eq!(babel_prefix(b"\xfe\xed\xfa\xce"), "MACHO-");
        assert_eq!(babel_prefix(b"\xca\xfe\xba\xbe\0\0\0\x02"), "JAVA-");
        assert_eq!(babel_prefix(b"APPL"), "MAC-");
        let d = detect(b"\x7fELF and nothing else");
        assert_eq!(d.source, Source::Fallback);
        assert!(d.ifids[0].as_str().starts_with("ELF-"));
        let d = detect(b"<html><head></head></html>");
        assert!(d.ifids[0].as_str().starts_with("HTML-"));
    }

    #[test]
    fn marker_in_binary() {
        let d = detect(b"\x7fELF\0\0UUID://1974A053-7DB0-4103-93A1-767C1382C0B7//\0");
        assert_eq!(d.source, Source::Marker);
        assert_eq!(d.ifids[0].as_str(), "1974A053-7DB0-4103-93A1-767C1382C0B7");
    }
}
