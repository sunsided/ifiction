//! `scan::find_markers` and `scan::find_ifiction` on arbitrary bytes.
//!
//! Invariants: no panic; every marker is a valid IFID, found as `UUID://<IFID>//` in the
//! bytes, without duplicates; every embedded record is a slice that reads to its index.

#![no_main]

use ifiction::{Ifid, read, scan};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let markers = scan::find_markers(bytes);
    for (i, ifid) in markers.iter().enumerate() {
        assert_eq!(Ifid::parse(ifid.as_str()).as_ref(), Ok(ifid));
        let marker = ifid.marker();
        assert!(
            bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
            "{marker} is not in the input"
        );
        assert!(!markers[..i].contains(ifid), "duplicate marker {ifid}");
    }
    for (xml, index) in scan::find_ifiction(bytes) {
        let reread = read::from_str(xml).expect("a found record reads");
        assert_eq!(reread, index);
    }
});
