//! `html::extract_ifid` and `html::inject` on arbitrary text.
//!
//! The input is the page; text after a final NUL byte, if it is a valid IFID, is the IFID
//! to inject (a fixed one otherwise), so plain HTML files work as seeds.
//!
//! Invariants: no panic; after a successful inject, the page yields the injected IFID from
//! its meta tag, and injecting again gives the same page.

#![no_main]

use ifiction::Ifid;
use ifiction::html::{self, HtmlSource};
use libfuzzer_sys::fuzz_target;

const DEFAULT_IFID: &str = "448E73DF-2D2F-47E7-A494-A46B40D4CFB3";

fuzz_target!(|input: &str| {
    let (page, ifid) = match input.rsplit_once('\0') {
        Some((page, id)) => match Ifid::parse(id) {
            Ok(ifid) => (page, ifid),
            Err(_) => (input, Ifid::parse(DEFAULT_IFID).unwrap()),
        },
        None => (input, Ifid::parse(DEFAULT_IFID).unwrap()),
    };
    let _ = html::extract_ifid(page);
    let Ok(out) = html::inject(page, &ifid) else {
        return;
    };
    assert_eq!(
        html::extract_ifid(&out),
        Some((ifid.clone(), HtmlSource::Meta)),
        "inject did not take\n{out}"
    );
    assert_eq!(
        html::inject(&out, &ifid).as_deref(),
        Ok(out.as_str()),
        "inject is not idempotent"
    );
});
