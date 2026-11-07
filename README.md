# ifiction

Treaty of Babel metadata for Rust games: IFIDs, iFiction records, and the ways to embed
them in native programs, WASM modules and web pages.

The [Treaty of Babel](https://babel.ifarchive.org/babel_rev11.html) is how the
interactive fiction world identifies and describes games. IFDB, the IF Archive and
interpreters use the IFID (a UUID, in capitals) and the iFiction record (an XML file with
title, author, genre, cover and so on). This workspace makes a Rust game take part:
it generates the record at build time, embeds it in the binary, and puts the IFID meta
tag into the game's HTML page.

## Layout

| Path                    | What                                                                  |
|-------------------------|-----------------------------------------------------------------------|
| `crates/ifiction`       | Library: `Ifid`, the record model, XML reader and writer, lint, HTML, detection |
| `crates/ifiction-build` | `build.rs` helper: `[package.metadata.ifiction]` to an embedded module |
| `crates/ifiction-cli`   | The `ifiction` command                                                |
| `examples/hello`        | A tiny game, built natively and for `wasm32-unknown-unknown`          |

## One IFID per format

An IFID names a work, not a file: rebuilds and bug fixes keep it. A port to another
format gets a new one. A native build has the format `executable` and a web build (WASM in
a page) has the format `html`, so a game needs two IFIDs. Linux, Windows and macOS builds
are all `executable` and share one. IFDB lists both IFIDs under the same game.

`ifiction-build` picks the IFID by target: `wasm32-unknown-unknown` is `html`, everything
else is `executable`. `IFICTION_FORMAT=html` (or any format name) overrides it.

## Quick start

1. Create the IFIDs. This adds them to `Cargo.toml` and keeps its formatting:

   ```sh
   ifiction init            # executable and html
   ```

2. Describe the game. Title, author, description and URL default to the `[package]` fields.

   ```toml
   [package.metadata.ifiction]
   title = "The Lighthouse"
   headline = "An Interactive Fiction"
   genre = "Mystery"
   language = "en"
   first-published = "2026"
   forgiveness = "Polite"     # Merciful, Polite, Tough, Nasty, Cruel
   release = 1
   cover = { path = "assets/cover.png", description = "A lighthouse at night." }
   description = """
   The first paragraph.

   Blank lines separate paragraphs."""

   [package.metadata.ifiction.ifid]
   executable = "B6C1A1D4-3B4E-4F0A-9E1F-2D2C9E2A7F10"
   html = "E2B7F0A9-5C3D-4E81-A6B2-7D9C4F1E0B35"

   [dependencies]
   ifiction = { version = "0.1", default-features = false }

   [build-dependencies]
   ifiction-build = "0.1"
   ```

3. Generate the module in `build.rs`:

   ```rust,ignore
   fn main() {
       if let Err(e) = ifiction_build::generate() {
           panic!("{e}");
       }
   }
   ```

   A record that breaks the Treaty (a bad language code, a cover smaller than 120 pixels,
   …) fails the build. Guideline problems become `cargo` warnings. IFIDs may be written in
   lowercase, as most UUID tools print them; the record, the marker and the HTML tag
   always use capitals, as the Treaty requires.

4. Include it:

   ```rust,ignore
   mod ifiction_meta {
       ifiction::include_generated!();
   }

   fn main() {
       if std::env::args().any(|a| a == "--ifid") {
           println!("{}", ifiction_meta::ifid());
       }
   }
   ```

   The module has `IFID`, `FORMAT`, `TITLE`, `AUTHOR`, `ifid()`, `ifiction_xml()` and the
   `#[used]` static `EMBEDDED`: `UUID://<IFID>//`, a newline, then the whole record. Call
   `ifid()` or `ifiction_xml()` somewhere, or the Windows linker drops the block (see
   Limits).

5. For the web build, put the IFID into the page:

   ```sh
   ifiction html inject index.html --manifest .
   ```

   This writes `<meta prefix="ifiction: …" property="ifiction:ifid" content="…">` into
   `<head>`, after `<meta charset>`. Running it again changes nothing.

Run `examples/hello` with `cargo run -p hello`, and `task verify:embed` to build it with
fat LTO for both targets and check what ends up in the binaries.

## The `ifiction` command

```text
ifiction new-ifid [-n N]                 print new IFIDs
ifiction init [--manifest DIR] [--formats executable,html]
ifiction ifid [-v] FILE...               IFIDs of any file, like babel -ifid
ifiction meta FILE|Cargo.toml [--format html]
ifiction lint FILE [--strict]            like babel -lint; FILE may be a game binary
ifiction identify FILE
ifiction html inject FILE [--ifid ID | --manifest DIR] [-o OUT|-]
ifiction html extract FILE
```

`ifid` follows babel's order: the record of an iFiction file; in HTML the meta tag, then a
`UUID://…//` marker, then Twine's `ifid` attribute; in other files markers and embedded
records; last the Treaty's MD5 fallback (`ELF-<MD5>`, `MZ-<MD5>`, `MACHO-<MD5>`,
`HTML-<MD5>`, or a bare MD5).

## Limits

- babel itself does not look for markers in native executables. It gives them the MD5
  fallback IFID. The embedded block is for this tool, for your own tooling and for IFDB
  submissions; the HTML meta tag is what babel reads.
- Read the block once, for example with `ifiction_meta::ifid()` behind a `--ifid` flag.
  `#[used]` keeps it from rustc, but linkers drop data that nothing refers to. Checked with
  fat LTO and stripping (`.github/workflows/embed-survival.yml`):

  | Target | Block kept when read | Block kept when never read |
  |---|---|---|
  | Linux (ELF) | yes | yes |
  | macOS (Mach-O) | yes | yes |
  | Windows (MSVC) | yes | **no**: `/OPT:REF` removes it |
  | `wasm32-unknown-unknown` | yes | yes |
- `wasm-custom-section = true` also writes the block into a WASM custom section named
  `ifiction`. That needs `#[unsafe(link_section)]`; the generated code allows
  `unsafe_code` for that item, which does not work under `#![forbid(unsafe_code)]`.
- Not supported yet: Blorb, IFDB lookups and `putific` uploads.

## Development

Tasks live in `Taskfile.dist.yaml` ([go-task](https://taskfile.dev)):

| Task | What |
|---|---|
| `task verify` | Formatting, clippy (all feature sets, wasm32), tests, embedding check |
| `task verify:embed` | Build the example with fat LTO, natively and for the web, and check the embedded metadata |
| `task lint:workflows` | actionlint and zizmor on `.github/` (needs docker and uv) |
| `task run -- <args>` | Run the `ifiction` CLI |
| `task release:dry -- X.Y.Z` | Show what a release would do ([cargo-release](https://github.com/crate-ci/cargo-release)) |
| `task release -- X.Y.Z` | Bump, stamp the changelog, commit, tag `vX.Y.Z` and push |

CI (`.github/workflows/ci.yml`) runs the same checks, plus an MSRV build on Rust 1.95.
A pushed `vX.Y.Z` tag runs `.github/workflows/release.yml`: a GitHub release with the
changelog section as notes, CLI archives for Linux, macOS and Windows, and the crates on
crates.io through trusted publishing (no stored token).
Actions are pinned by commit SHA and check out without persisted credentials.

## License

Licensed under any of

- European Union Public Licence 1.2 ([LICENSE-EUPL-1.2](LICENSE-EUPL-1.2))
- MIT ([LICENSE-MIT](LICENSE-MIT))
- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
