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
fn lowercase_manifest_ifids() {
    let dir = temp_dir("lowercase");
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[package.metadata.ifiction.ifid]\nexecutable = \"1974a053-7db0-4103-93a1-767c1382c0b7\"\nhtml = \"448e73df-2d2f-47e7-a494-a46b40d4cfb3\"\n",
    )
    .unwrap();
    let dir_arg = dir.to_str().unwrap();
    assert!(ifiction(&["init", "--manifest", dir_arg]).status.success());
    let xml = stdout(&ifiction(&["meta", dir_arg]));
    assert!(
        xml.contains("<ifid>1974A053-7DB0-4103-93A1-767C1382C0B7</ifid>"),
        "{xml}"
    );

    let page = dir.join("index.html");
    std::fs::write(&page, "<html><head></head></html>").unwrap();
    let page_arg = page.to_str().unwrap();
    assert!(
        ifiction(&["html", "inject", page_arg, "--manifest", dir_arg])
            .status
            .success()
    );
    assert!(
        std::fs::read_to_string(&page)
            .unwrap()
            .contains(r#"content="448E73DF-2D2F-47E7-A494-A46B40D4CFB3""#)
    );
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

/// Answers one HTTP request with `body`; returns the server's URL and the request it saw.
fn serve_once(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().unwrap();
            }
            request.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let mut payload = vec![0; length];
        reader.read_exact(&mut payload).unwrap();
        request.push_str(&String::from_utf8_lossy(&payload));
        let mut stream = stream;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        request
    });
    (url, handle)
}

fn ifiction_env(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ifiction"));
    cmd.args(args)
        .env_remove("IFDB_USERNAME")
        .env_remove("IFDB_PASSWORD");
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().expect("runs")
}

#[test]
fn ifdb_fetch_sends_ifid_and_user_agent() {
    let (url, server) = serve_once(
        "<ifindex version=\"1.0\"><story><identification><ifid>ZCODE-1-2-3</ifid><format>zcode</format></identification><bibliographic><title>T</title><author>A</author></bibliographic></story></ifindex>",
    );
    let o = ifiction_env(&["ifdb", "fetch", "zcode-1-2-3"], &[("IFDB_URL", &url)]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(stdout(&o).contains("<ifid>ZCODE-1-2-3</ifid>"));
    let request = server.join().unwrap();
    assert!(
        request.starts_with("GET /viewgame?ifiction&ifid=ZCODE-1-2-3 "),
        "{request}"
    );
    assert!(
        request
            .to_ascii_lowercase()
            .contains("user-agent: ifiction-cli/")
    );
}

#[test]
fn ifdb_fetch_reports_errors() {
    let (url, server) = serve_once(
        "<viewgame xmlns=\"http://ifdb.org/api/xmlns\"><errorCode>notFound</errorCode><errorMessage>No game</errorMessage></viewgame>",
    );
    let o = ifiction_env(&["ifdb", "fetch", "ZCODE-1-2-3"], &[("IFDB_URL", &url)]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("IFDB: notFound: No game"));
    server.join().unwrap();
}

#[test]
fn ifdb_put_uploads_multipart() {
    let (url, server) = serve_once(
        "<putific xmlns=\"http://ifdb.org/api/xmlns\"><ok/><created/><tuid>t123</tuid><viewUrl>https://ifdb.org/viewgame?id=t123</viewUrl></putific>",
    );
    let o = ifiction_env(
        &[
            "ifdb",
            "put",
            FIXTURE,
            "--username",
            "me@example.org",
            "--image-copyright-status",
            "public domain",
        ],
        &[("IFDB_URL", &url), ("IFDB_PASSWORD", "s3cret")],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        stdout(&o),
        "created\nTUID: t123\nhttps://ifdb.org/viewgame?id=t123\n"
    );
    let request = server.join().unwrap();
    assert!(request.starts_with("POST /putific "), "{request}");
    assert!(request.contains("multipart/form-data; boundary="));
    assert!(request.contains("name=\"username\"\r\n\r\nme@example.org\r\n"));
    assert!(request.contains("name=\"password\"\r\n\r\ns3cret\r\n"));
    assert!(request.contains("name=\"imageCopyrightStatus\"\r\n\r\npublic domain\r\n"));
    assert!(request.contains("<ifid>ZCODE-18-860904</ifid>"));
}

#[test]
fn ifdb_put_dry_run_and_missing_credentials() {
    let o = ifiction_env(
        &["ifdb", "put", FIXTURE, "--dry-run"],
        &[("IFDB_PASSWORD", "s3cret")],
    );
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = stdout(&o);
    assert!(out.starts_with("POST https://ifdb.org/putific\n"), "{out}");
    assert!(out.contains("password: (hidden)"));
    assert!(!out.contains("s3cret"));

    let o = ifiction_env(
        &["ifdb", "put", FIXTURE],
        &[("IFDB_URL", "http://127.0.0.1:9")],
    );
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("IFDB_USERNAME"));
}
