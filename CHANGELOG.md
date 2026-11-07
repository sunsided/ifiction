# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `ifiction`: IFIDs (parse, generate, `UUID://…//` markers), the iFiction record model,
  a deterministic XML writer and a tolerant reader, lint rules of the Treaty of Babel
  rev. 11, the HTML IFID meta tag, cover image probing, and IFID detection with babel's
  MD5 fallbacks.
- `ifiction-build`: generates an embeddable module from `[package.metadata.ifiction]`,
  with one IFID per format (`executable` for native targets, `html` for
  `wasm32-unknown-unknown`).
- `ifiction-cli`: the `ifiction` command (`new-ifid`, `init`, `ifid`, `meta`, `lint`,
  `identify`, `html inject`, `html extract`).
