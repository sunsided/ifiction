# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-11-07

### Added

- `ifiction ifdb fetch`: a game's IFDB listing as iFiction XML or JSON, by IFID, by the IFID
  of a file, or by TUID.
- `ifiction ifdb put`: create or update an IFDB listing through the putific API, with cover,
  links and copyright fields, and `--dry-run`. Tested against a local server only.
- The `quest` format of Treaty revision 13.
- Generated records name the compiler (`rustc` and its version) in the attached release and
  the target triple in the `<executable>` or `<html>` section.
- Property tests (proptest) for IFIDs, markers, dates, the writer and reader, lint, HTML
  injection, detection, cover probing and `ifiction-build`, and cargo-fuzz targets for the
  reader, detection, marker scanning, HTML, cover probing and generated records, with a
  `fuzz.yml` smoke workflow and `task fuzz` / `task fuzz:smoke`.
- CI compares IFIDs, `-verify` and `-lint` with babel 0.75, the Treaty's reference tool, and
  the test suite includes babel's own test files.

### Changed

- IFID detection follows babel 0.75 and Treaty revision 13: a `UUID://…//` marker counts only
  in capitals, and the MD5 fallback prefixes use babel's magic numbers (64-bit macOS binaries
  and WASM modules get a bare MD5, `CAFEBABE` is always `JAVA`, Amiga is `000003E7`).
- iFiction records without an XML declaration, as IFDB serves them, are recognized as such.
- HTML tags end at the next `<` or `>`, as in babel; quotes are not special. A `>` inside an
  attribute value now ends the tag, as it does for babel.

### Fixed

- `lint::check` panicked (debug) or misjudged the aspect ratio (release) on cover sizes from
  2^31 pixels, which an untrusted record can contain.
- `PartialDate::from_unix_seconds` gave invalid dates such as `9999-02-29` beyond the year
  9999; it now clamps to `0000-01-01` and `9999-12-31`.
- `html::inject` was not idempotent on malformed HTML (a stray quote in an earlier tag, or
  `<head>` inside an attribute), and the injected IFID could not be read back.
- `<br/>` in a field other than the description was read as a line break, so a write and read
  changed the record; it is a space there now.

## [0.1.0] - 2026-11-07

### Added

- `ifiction`: IFIDs (parse, generate, `UUID://…//` markers), the iFiction record model,
  a deterministic XML writer and a tolerant reader, lint rules of the Treaty of Babel
  rev. 11, the HTML IFID meta tag, cover image probing, and IFID detection with babel's
  MD5 fallbacks. Without default features it has no dependencies.
- `ifiction-build`: generates an embeddable module from `[package.metadata.ifiction]`,
  with one IFID per format (`executable` for native targets, `html` for
  `wasm32-unknown-unknown`). IFIDs may be written in lowercase; the output uses capitals.
- `ifiction-cli`: the `ifiction` command (`new-ifid`, `init`, `ifid`, `meta`, `lint`,
  `identify`, `html inject`, `html extract`).
- A manual workflow that checks on Linux, macOS and Windows that the embedded block
  survives fat LTO and stripping. The MSVC linker drops a block that the game never reads.

[Unreleased]: https://github.com/sunsided/ifiction/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/sunsided/ifiction/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/sunsided/ifiction/releases/tag/v0.1.0
