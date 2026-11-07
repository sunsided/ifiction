//! `detect::detect` on arbitrary bytes.
//!
//! Invariants: no panic; at least one IFID, and every IFID is valid.

#![no_main]

use ifiction::Ifid;
use ifiction::detect::detect;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let d = detect(bytes);
    assert!(!d.ifids.is_empty(), "no IFID for {d:?}");
    for ifid in &d.ifids {
        assert_eq!(Ifid::parse(ifid.as_str()).as_ref(), Ok(ifid));
    }
});
