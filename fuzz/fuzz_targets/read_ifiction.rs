//! `read::from_str` and `read::ifids` on arbitrary text.
//!
//! Invariants: no panic (also not in `lint`); a record that reads writes deterministically,
//! and what it writes reads back to the same index.

#![no_main]

use ifiction::{lint, read, write};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|xml: &str| {
    let _ = read::ifids(xml);
    let Ok(index) = read::from_str(xml) else {
        return;
    };
    let _ = lint::check_index(&index);
    let out = write::to_string(&index);
    assert_eq!(
        out,
        write::to_string(&index),
        "writing is not deterministic"
    );
    let again =
        read::from_str(&out).unwrap_or_else(|e| panic!("written record does not read: {e}\n{out}"));
    assert_eq!(again, index, "round trip changed the record\n{out}");
    // Every IFID of the index is one that `ifids` finds too.
    let ids = read::ifids(&out).expect("written record has valid IFIDs");
    let all: Vec<_> = index
        .stories
        .iter()
        .flat_map(|s| s.identification.ifids.iter().cloned())
        .collect();
    assert_eq!(ids, all);
});
