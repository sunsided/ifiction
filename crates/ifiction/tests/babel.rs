//! Conformance with babel, the reference tool of the Treaty of Babel: the IFIDs that
//! `ifiction::detect` finds must equal babel's output for babel's own test files
//! (`tests/fixtures/babel`, see its README).

#![cfg(feature = "detect")]

use std::path::Path;

fn fixtures(dir: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/babel")
        .join(dir)
}

/// The `IFID:` lines of babel's `expected` file, in order.
fn expected_ifids(dir: &str) -> Vec<String> {
    std::fs::read_to_string(fixtures(dir).join("expected"))
        .expect("expected")
        .lines()
        .filter_map(|l| l.strip_prefix("IFID: "))
        .map(str::to_owned)
        .collect()
}

fn detected(dir: &str, file: &str) -> String {
    let bytes = std::fs::read(fixtures(dir).join(file)).expect(file);
    ifiction::detect::detect(&bytes).ifids[0].to_string()
}

#[test]
fn generic_files_match_babel() {
    // The order of babel's test Makefile.
    let files = [
        "noifid.bin",
        "ifid.bin",
        "textifid.txt",
        "script.sh",
        "scriptifid.sh",
        "casetest.sh",
        "ifid-at-start.bin",
        "ifid-at-end.bin",
    ];
    let expected = expected_ifids("generic");
    assert_eq!(expected.len(), files.len());
    for (file, want) in files.iter().zip(&expected) {
        assert_eq!(&detected("generic", file), want, "{file}");
    }
}

#[test]
fn html_files_match_babel() {
    let files = [
        "Test-Game-Meta.html",
        "Test-Game-UUID.html",
        "Test-Game-Legacy.html",
    ];
    let expected = expected_ifids("html");
    assert_eq!(expected.len(), files.len());
    for (file, want) in files.iter().zip(&expected) {
        assert_eq!(&detected("html", file), want, "{file}");
    }
}
