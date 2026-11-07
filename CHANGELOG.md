# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/sunsided/ifiction/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/sunsided/ifiction/releases/tag/v0.1.0
