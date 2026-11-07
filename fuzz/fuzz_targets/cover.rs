//! `cover::probe` on arbitrary bytes.
//!
//! Invariant: no panic.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = ifiction::cover::probe(bytes);
});
