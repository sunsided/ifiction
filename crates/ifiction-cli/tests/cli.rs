//! Runs the `ifiction` binary.

use std::path::PathBuf;
use std::process::{Command, Output};

fn ifiction(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ifiction"))
        .args(args)
        .output()
        .expect("runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ifiction/tests/fixtures/legacy.iFiction"
);

#[test]
fn new_ifid() {
    let o = ifiction(&["new-ifid", "-n", "2"]);
    assert!(o.status.success());
    let lines: Vec<String> = stdout(&o).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 2);
    assert_ne!(lines[0], lines[1]);
    assert!(lines.iter().all(|l| ifiction::Ifid::parse(l).is_ok()));
}

#[test]
fn ifid_of_ifiction_file() {
    let o = ifiction(&["ifid", FIXTURE]);
    assert_eq!(
        stdout(&o),
        "IFID: ZCODE-18-860904\nIFID: ZCODE-15-851108\nIFID: 1974A053-7DB0-4103-93A1-767C1382C0B7\n"
    );
}

#[test]
fn lint_reports_and_fails() {
    let o = ifiction(&["lint", FIXTURE]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(stdout(&o), "Verified ZCODE-18-860904\n");

    let dir = temp_dir("lint");
    let bad = dir.join("bad.iFiction");
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap()
        .replace("<language>en-US</language>", "<language>English</language>");
    std::fs::write(&bad, text).unwrap();
    let o = ifiction(&["lint", bad.to_str().unwrap()]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("error: story[1]/bibliographic/language"));
}

#[test]
fn init_then_meta_then_html() {
    let dir = temp_dir("init");
    let manifest = dir.join("Cargo.toml");
    std::fs::write(
        &manifest,
        "[package]\nname = \"demo\" # the name\nversion = \"0.1.0\"\nauthors = [\"Jane Doe <jane@example.org>\"]\n\n[package.metadata.ifiction]\ngenre = \"Fantasy\"\n",
    )
    .unwrap();
    let dir_arg = dir.to_str().unwrap();

    assert!(ifiction(&["init", "--manifest", dir_arg]).status.success());
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(
        text.contains("name = \"demo\" # the name"),
        "keeps formatting"
    );
    assert!(text.contains("[package.metadata.ifiction.ifid]"));
    // A second run keeps the IFIDs.
    assert!(ifiction(&["init", "--manifest", dir_arg]).status.success());
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);

    let o = ifiction(&["meta", dir_arg, "--format", "html"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let xml = stdout(&o);
    assert!(xml.contains("<format>html</format>"));
    assert!(xml.contains("<author>Jane Doe</author>"));
    assert!(xml.contains("<genre>Fantasy</genre>"));
    let html_ifid = ifiction::read::ifids(&xml).unwrap()[0].to_string();

    let page = dir.join("index.html");
    std::fs::write(
        &page,
        "<html>\n<head>\n<title>x</title>\n</head>\n</html>\n",
    )
    .unwrap();
    let page_arg = page.to_str().unwrap();
    assert!(
        ifiction(&["html", "inject", page_arg, "--manifest", dir_arg])
            .status
            .success()
    );
    assert_eq!(
        stdout(&ifiction(&["html", "extract", page_arg])).trim(),
        html_ifid
    );
    assert_eq!(
        stdout(&ifiction(&["ifid", page_arg])).trim(),
        format!("IFID: {html_ifid}")
    );
}
