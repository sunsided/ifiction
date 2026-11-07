//! Finds IFIDs and iFiction records inside arbitrary bytes: story files, native
//! executables, WASM modules.

use crate::Ifid;

const MARKER_START: &[u8] = b"UUID://";
const MARKER_END: &[u8] = b"//";

/// All `UUID://…//` markers, in file order, without duplicates.
///
/// The Treaty uses the first one. Like babel, only digits, capitals and hyphens count:
/// `UUID://1974a053-…//` is not a marker.
pub fn find_markers(bytes: &[u8]) -> Vec<Ifid> {
    let mut out: Vec<Ifid> = Vec::new();
    let mut from = 0;
    while let Some(pos) = find(&bytes[from..], MARKER_START) {
        let start = from + pos + MARKER_START.len();
        let len = bytes[start..]
            .iter()
            .take_while(|b| b.is_ascii_digit() || b.is_ascii_uppercase() || **b == b'-')
            .count();
        from = start;
        if bytes[start + len..].starts_with(MARKER_END)
            && let Ok(s) = std::str::from_utf8(&bytes[start..start + len])
            && let Ok(ifid) = Ifid::parse(s)
            && !out.contains(&ifid)
        {
            out.push(ifid);
        }
    }
    out
}

/// Every embedded iFiction record that parses, in file order.
///
/// A candidate runs from `<?xml` (or `<ifindex`) to `</ifindex>`. Candidates that do not
/// parse are skipped, so string constants of XML-writing code do not count.
#[cfg(feature = "read")]
pub fn find_ifiction(bytes: &[u8]) -> Vec<(&str, crate::IfIndex)> {
    const END: &[u8] = b"</ifindex>";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = find(&bytes[from..], b"<ifindex") {
        let root = from + pos;
        from = root + 1;
        let Some(end) = find(&bytes[root..], END).map(|e| root + e + END.len()) else {
            break;
        };
        // Start at an XML declaration right before the root, if there is one.
        let window = root.saturating_sub(128);
        let start = rfind(&bytes[window..root], b"<?xml").map_or(root, |p| window + p);
        let Ok(xml) = std::str::from_utf8(&bytes[start..end]) else {
            continue;
        };
        if let Ok(index) = crate::read::from_str(xml) {
            out.push((xml, index));
            from = end;
        }
    }
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(feature = "read")]
fn rfind(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers() {
        let bytes = b"\0\0UUID://1974a053-7DB0-4103-93A1-767C1382C0B7//junk UUID://bad UUID://ZCODE-1-2-3// UUID://1974A053-7DB0-4103-93A1-767C1382C0B7//";
        // The lowercase one does not count.
        let found: Vec<String> = find_markers(bytes)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            found,
            ["ZCODE-1-2-3", "1974A053-7DB0-4103-93A1-767C1382C0B7"]
        );
    }

    #[test]
    fn marker_at_end_without_terminator() {
        assert!(find_markers(b"UUID://ABCDEFGH").is_empty());
        assert!(find_markers(b"UUID://").is_empty());
    }
}
