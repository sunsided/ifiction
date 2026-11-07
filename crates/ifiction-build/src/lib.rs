//! Embeds Treaty of Babel metadata into a Rust game at build time.
//!
//! Put the metadata into `Cargo.toml`:
//!
//! ```toml
//! [package.metadata.ifiction]
//! title = "The Lighthouse"        # default: the package name
//! genre = "Mystery"
//! language = "en"
//!
//! [package.metadata.ifiction.ifid]   # `ifiction init` fills this in
//! executable = "1974A053-7DB0-4103-93A1-767C1382C0B7"
//! html = "448E73DF-2D2F-47E7-A494-A46B40D4CFB3"
//! ```
//!
//! and call [`generate`] from `build.rs`:
//!
//! ```no_run
//! // build.rs, inside `fn main`:
//! ifiction_build::generate().expect("iFiction metadata");
//! ```
//!
//! Then `ifiction::include_generated!()` brings the result into the crate.
//!
//! One IFID per format: the Treaty gives a port to another format a new IFID. A build for
//! `wasm32-unknown-unknown` is a web game, format `html`; every other target is a native
//! program, format `executable`, and all native platforms share its IFID. The environment
//! variable `IFICTION_FORMAT` overrides the choice.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ifiction::lint::{self, Diagnostic, Severity};
use ifiction::model::{
    Auxiliary, Colophon, Contacts, Cover, Forgiveness, Format, FormatSection, IfIndex, PartialDate,
    Release, Releases, Story,
};
use ifiction::{Ifid, write};
use serde::Deserialize;

/// The name of the generated module in `OUT_DIR`.
pub const GENERATED_RS: &str = "ifiction.rs";
/// The embedded block in `OUT_DIR`: the marker, a newline, the XML.
pub const GENERATED_BIN: &str = "ifiction.bin";

/// Why the metadata could not be built.
#[derive(Debug)]
pub enum Error {
    Env(&'static str),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Toml {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    NoPackage(PathBuf),
    MissingIfid {
        format: String,
    },
    Ifid {
        format: String,
        source: ifiction::IfidError,
    },
    Value(String),
    Cover {
        path: PathBuf,
        source: ifiction::cover::CoverError,
    },
    Lint(Vec<Diagnostic>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Env(var) => write!(
                f,
                "environment variable {var} is not set; call this from a build script"
            ),
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Toml { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NoPackage(path) => write!(f, "{}: no [package] table", path.display()),
            Self::MissingIfid { format } => write!(
                f,
                "no IFID for format {format:?} in [package.metadata.ifiction.ifid]; run `ifiction init` to create one"
            ),
            Self::Ifid { format, source } => {
                write!(f, "[package.metadata.ifiction.ifid] {format}: {source}")
            }
            Self::Value(message) => write!(f, "[package.metadata.ifiction] {message}"),
            Self::Cover { path, source } => write!(f, "cover {}: {source}", path.display()),
            Self::Lint(diagnostics) => {
                f.write_str("the iFiction record breaks the Treaty:")?;
                for d in diagnostics {
                    write!(f, "\n  {d}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Toml { source, .. } => Some(source.as_ref()),
            Self::Ifid { source, .. } => Some(source),
            Self::Cover { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// `[package.metadata.ifiction]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Metadata {
    /// One IFID per format name, such as `executable` and `html`. Lowercase is accepted;
    /// the record and the embedded block use capitals, as the Treaty requires.
    #[serde(default)]
    pub ifid: BTreeMap<String, String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub headline: Option<String>,
    pub genre: Option<String>,
    pub group: Option<String>,
    /// Paragraphs are separated by blank lines.
    pub description: Option<String>,
    pub first_published: Option<String>,
    pub series: Option<String>,
    pub series_number: Option<u32>,
    pub forgiveness: Option<String>,
    /// Default: the package homepage.
    pub url: Option<String>,
    pub author_email: Option<String>,
    /// The release number, a non-negative integer.
    pub release: Option<u32>,
    pub cover: Option<CoverSpec>,
    #[serde(default)]
    pub resources: Vec<ResourceSpec>,
    /// Also put the block into a WASM custom section named `ifiction`.
    #[serde(default)]
    pub wasm_custom_section: bool,
}

/// `cover = { path = "…", description = "…" }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CoverSpec {
    /// Relative to the package directory.
    pub path: PathBuf,
    pub description: Option<String>,
}

/// `resources = [{ leafname = "…", description = "…" }]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResourceSpec {
    pub leafname: String,
    pub description: String,
}

/// The package fields that serve as defaults.
#[derive(Debug, Clone, Default)]
pub struct PackageInfo {
    pub name: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub version: String,
}

impl PackageInfo {
    /// From the variables Cargo sets for build scripts.
    pub fn from_env() -> Result<Self, Error> {
        let var = |k: &'static str| std::env::var(k).ok().filter(|v| !v.is_empty());
        Ok(Self {
            name: var("CARGO_PKG_NAME").ok_or(Error::Env("CARGO_PKG_NAME"))?,
            authors: var("CARGO_PKG_AUTHORS")
                .map(|a| a.split(':').map(str::to_owned).collect())
                .unwrap_or_default(),
            description: var("CARGO_PKG_DESCRIPTION"),
            homepage: var("CARGO_PKG_HOMEPAGE"),
            version: var("CARGO_PKG_VERSION").unwrap_or_default(),
        })
    }

    /// From a `Cargo.toml`; `field.workspace = true` is looked up in the workspace root.
    pub fn from_manifest(manifest: &Path) -> Result<Self, Error> {
        let doc = read_toml(manifest)?;
        let package = doc
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| Error::NoPackage(manifest.to_owned()))?;
        let mut workspace: Option<toml::Table> = None;
        let mut field = |key: &str| -> Result<Option<toml::Value>, Error> {
            let Some(v) = package.get(key) else {
                return Ok(None);
            };
            let inherited = v
                .as_table()
                .and_then(|t| t.get("workspace"))
                .and_then(toml::Value::as_bool)
                == Some(true);
            if !inherited {
                return Ok(Some(v.clone()));
            }
            if workspace.is_none() {
                workspace = Some(find_workspace_package(manifest)?);
            }
            Ok(workspace.as_ref().and_then(|w| w.get(key)).cloned())
        };
        let string = |v: Option<toml::Value>| v.and_then(|v| v.as_str().map(str::to_owned));
        Ok(Self {
            name: string(field("name")?).unwrap_or_default(),
            authors: field("authors")?
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|a| a.as_str().map(str::to_owned))
                .collect(),
            description: string(field("description")?),
            homepage: string(field("homepage")?),
            version: string(field("version")?).unwrap_or_default(),
        })
    }
}

fn find_workspace_package(manifest: &Path) -> Result<toml::Table, Error> {
    let start = manifest.parent().unwrap_or(Path::new("."));
    for dir in start.ancestors().skip(1) {
        let candidate = dir.join("Cargo.toml");
        if !candidate.is_file() {
            continue;
        }
        let doc = read_toml(&candidate)?;
        if let Some(ws) = doc.get("workspace").and_then(toml::Value::as_table) {
            return Ok(ws
                .get("package")
                .and_then(toml::Value::as_table)
                .cloned()
                .unwrap_or_default());
        }
    }
    Ok(toml::Table::new())
}

fn read_toml(path: &Path) -> Result<toml::Table, Error> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    text.parse::<toml::Table>().map_err(|source| Error::Toml {
        path: path.to_owned(),
        source: Box::new(source),
    })
}

/// Reads `[package.metadata.ifiction]`; an absent table gives the defaults.
pub fn read_metadata(manifest: &Path) -> Result<Metadata, Error> {
    let doc = read_toml(manifest)?;
    let Some(table) = doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("ifiction"))
    else {
        return Ok(Metadata::default());
    };
    table.clone().try_into().map_err(|source| Error::Toml {
        path: manifest.to_owned(),
        source: Box::new(source),
    })
}

/// The format of a build target: `html` for `wasm32-unknown-unknown`, else `executable`.
pub fn format_for_target(arch: &str, os: &str) -> Format {
    if arch.starts_with("wasm") && os == "unknown" {
        Format::Html
    } else {
        Format::Executable
    }
}

/// Today in UTC, or the day of `SOURCE_DATE_EPOCH` for reproducible builds.
pub fn originated() -> PartialDate {
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64)
        });
    PartialDate::from_unix_seconds(secs)
}

/// A built record and the files it refers to.
#[derive(Debug, Clone)]
pub struct Built {
    pub story: Story,
    /// The cover image, if the metadata names one.
    pub cover_path: Option<PathBuf>,
    /// Warnings of the Treaty's guidelines. Errors make [`build_story`] fail.
    pub warnings: Vec<Diagnostic>,
}

/// Builds the record for one format from the metadata and the package defaults.
pub fn build_story(
    package_dir: &Path,
    meta: &Metadata,
    package: &PackageInfo,
    format: Format,
    originated: PartialDate,
) -> Result<Built, Error> {
    let ifid_text = meta
        .ifid
        .get(format.as_str())
        .ok_or_else(|| Error::MissingIfid {
            format: format.to_string(),
        })?;
    let ifid = Ifid::parse_lenient(ifid_text).map_err(|source| Error::Ifid {
        format: format.to_string(),
        source,
    })?;

    let title = meta.title.clone().unwrap_or_else(|| package.name.clone());
    let author = meta.author.clone().unwrap_or_else(|| {
        let names: Vec<String> = package.authors.iter().map(|a| strip_email(a)).collect();
        if names.is_empty() {
            "Anonymous".to_owned()
        } else {
            names.join(" and ")
        }
    });
    let mut story = Story::new(ifid, format, title, author);

    let b = &mut story.bibliographic;
    b.language.clone_from(&meta.language);
    b.headline.clone_from(&meta.headline);
    b.genre.clone_from(&meta.genre);
    b.group.clone_from(&meta.group);
    b.description = meta
        .description
        .as_deref()
        .or(package.description.as_deref())
        .map(paragraphs);
    b.first_published = meta
        .first_published
        .as_deref()
        .map(|d| {
            d.parse()
                .map_err(|e| Error::Value(format!("first-published: {e}")))
        })
        .transpose()?;
    b.series.clone_from(&meta.series);
    b.series_number = meta.series_number;
    b.forgiveness = meta
        .forgiveness
        .as_deref()
        .map(|f| {
            f.parse::<Forgiveness>()
                .map_err(|e| Error::Value(format!("forgiveness: {e}")))
        })
        .transpose()?;

    story.resources = meta
        .resources
        .iter()
        .map(|r| Auxiliary {
            leafname: r.leafname.clone(),
            description: r.description.clone(),
        })
        .collect();

    let url = meta.url.clone().or_else(|| package.homepage.clone());
    if url.is_some() || meta.author_email.is_some() {
        story.contacts = Some(Contacts {
            url,
            author_email: meta.author_email.clone(),
        });
    }

    let mut cover_path = None;
    if let Some(spec) = &meta.cover {
        let path = package_dir.join(&spec.path);
        let bytes = std::fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        let (format, width, height) =
            ifiction::cover::probe(&bytes).map_err(|source| Error::Cover {
                path: path.clone(),
                source,
            })?;
        story.cover = Some(Cover {
            format,
            width,
            height,
            description: spec.description.clone(),
        });
        cover_path = Some(path);
    }

    story.releases = Some(Releases {
        attached: Some(Release {
            release_date: originated,
            version: meta.release,
            compiler: None,
            compiler_version: None,
        }),
        history: Vec::new(),
    });
    story.colophon = Some(Colophon {
        generator: "ifiction-build".to_owned(),
        generator_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        originated,
    });

    let (errors, warnings): (Vec<_>, Vec<_>) = lint::check(&story)
        .into_iter()
        .partition(|d| d.severity == Severity::Error);
    if !errors.is_empty() {
        return Err(Error::Lint(errors));
    }
    Ok(Built {
        story,
        cover_path,
        warnings,
    })
}

/// How this build is made: the target and the compiler.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildInfo {
    /// The target triple, such as `x86_64-unknown-linux-gnu`.
    pub target: Option<String>,
    /// The rustc version, such as `1.99.1`.
    pub rustc_version: Option<String>,
}

impl BuildInfo {
    /// From the variables Cargo sets for build scripts (`TARGET`, `RUSTC`).
    pub fn from_env() -> Self {
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
        Self {
            target: std::env::var("TARGET").ok().filter(|t| !t.is_empty()),
            rustc_version: rustc_version(&rustc),
        }
    }
}

/// `rustc 1.99.1 (58cf8f9c3 2026-10-13)` gives `1.99.1`.
fn rustc_version(rustc: &str) -> Option<String> {
    let out = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    parse_rustc_version(&text)
}

fn parse_rustc_version(text: &str) -> Option<String> {
    let mut words = text.split_whitespace();
    (words.next()? == "rustc").then_some(())?;
    words.next().map(str::to_owned)
}

/// Adds the build to a record: rustc as the compiler of the attached release, and the
/// target triple in the format section (`<executable>` or `<html>`).
///
/// The Treaty lists these two sections without defining their contents, and leaves each
/// section to its design system (section 5.10), so `<target>` is this crate's convention.
pub fn add_build_info(story: &mut Story, build: &BuildInfo) {
    if let Some(version) = &build.rustc_version {
        let release = story
            .releases
            .get_or_insert_with(Releases::default)
            .attached
            .as_mut();
        if let Some(release) = release {
            release.compiler = Some("rustc".to_owned());
            release.compiler_version = Some(version.clone());
        }
    }
    if let Some(target) = &build.target
        && matches!(
            story.identification.format,
            Format::Executable | Format::Html
        )
    {
        story.format_section = Some(FormatSection {
            name: story.identification.format.to_string(),
            entries: vec![("target".to_owned(), target.clone())],
        });
    }
}

/// What [`generate`] wrote.
#[derive(Debug, Clone)]
pub struct Generated {
    pub ifid: Ifid,
    pub format: Format,
    /// `$OUT_DIR/ifiction.rs`.
    pub module: PathBuf,
    /// `$OUT_DIR/<IFID>.iFiction`.
    pub ifiction: PathBuf,
    /// `$OUT_DIR/<IFID>.png` or `.jpg`, if there is a cover.
    pub cover: Option<PathBuf>,
}

/// Reads the metadata of the package being built and writes the generated module.
///
/// Call it from `build.rs`. It prints `cargo::rerun-if-…` lines and the Treaty's guideline
/// warnings as `cargo::warning`.
pub fn generate() -> Result<Generated, Error> {
    let var = |k: &'static str| std::env::var(k).map_err(|_| Error::Env(k));
    let package_dir = PathBuf::from(var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(var("OUT_DIR")?);
    let manifest = package_dir.join("Cargo.toml");

    println!("cargo::rerun-if-changed={}", manifest.display());
    println!("cargo::rerun-if-env-changed=IFICTION_FORMAT");
    println!("cargo::rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let format = match std::env::var("IFICTION_FORMAT") {
        Ok(f) if !f.is_empty() => Format::from(f.as_str()),
        _ => format_for_target(
            &var("CARGO_CFG_TARGET_ARCH")?,
            &std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default(),
        ),
    };
    let meta = read_metadata(&manifest)?;
    let package = PackageInfo::from_env()?;
    let mut built = build_story(&package_dir, &meta, &package, format.clone(), originated())?;
    add_build_info(&mut built.story, &BuildInfo::from_env());
    for w in &built.warnings {
        println!("cargo::warning=iFiction: {}: {}", w.path, w.message);
    }
    if let Some(cover) = &built.cover_path {
        println!("cargo::rerun-if-changed={}", cover.display());
    }

    let ifid = built
        .story
        .primary_ifid()
        .cloned()
        .expect("build_story sets one");
    let xml = write::to_string(&IfIndex::single(built.story.clone()));
    let mut block = format!("{}\n", ifid.marker()).into_bytes();
    let xml_offset = block.len();
    block.extend_from_slice(xml.as_bytes());

    let write_file = |path: &Path, bytes: &[u8]| {
        std::fs::write(path, bytes).map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })
    };
    write_file(&out_dir.join(GENERATED_BIN), &block)?;
    let module = out_dir.join(GENERATED_RS);
    write_file(
        &module,
        module_source(
            &built.story,
            &ifid,
            block.len(),
            xml_offset,
            meta.wasm_custom_section,
        )
        .as_bytes(),
    )?;
    let ifiction = out_dir.join(format!("{ifid}.iFiction"));
    write_file(&ifiction, xml.as_bytes())?;
    let cover = match (&built.cover_path, &built.story.cover) {
        (Some(src), Some(c)) => {
            let dst = out_dir.join(format!("{ifid}.{}", c.format.as_str()));
            std::fs::copy(src, &dst).map_err(|source| Error::Io {
                path: src.clone(),
                source,
            })?;
            Some(dst)
        }
        _ => None,
    };

    Ok(Generated {
        ifid,
        format,
        module,
        ifiction,
        cover,
    })
}

/// The Rust source of the generated module.
fn module_source(
    story: &Story,
    ifid: &Ifid,
    len: usize,
    xml_offset: usize,
    wasm_section: bool,
) -> String {
    let marker_len = ifid.marker().len();
    let bin = format!("concat!(env!(\"OUT_DIR\"), \"/{GENERATED_BIN}\")");
    let mut s = String::new();
    let _ = write!(
        s,
        r#"// @generated by ifiction-build {version}. Do not edit.

/// The IFID of this build.
#[allow(dead_code)]
pub const IFID: &str = {ifid:?};
/// The iFiction format of this build: `executable` or `html`.
#[allow(dead_code)]
pub const FORMAT: &str = {format:?};
/// The title of the story.
#[allow(dead_code)]
pub const TITLE: &str = {title:?};
/// The author of the story.
#[allow(dead_code)]
pub const AUTHOR: &str = {author:?};

/// `UUID://<IFID>//`, a newline, then the iFiction record.
#[allow(dead_code)]
#[used]
pub static EMBEDDED: [u8; {len}] = *include_bytes!({bin});

/// The embedded block. Reading it keeps it in the binary, even with LTO.
#[allow(dead_code)]
pub fn embedded() -> &'static [u8] {{
    ::core::hint::black_box(&EMBEDDED)
}}

/// The IFID, read from the embedded block.
#[allow(dead_code)]
pub fn ifid() -> &'static str {{
    match ::core::str::from_utf8(&embedded()[7..{ifid_end}]) {{
        Ok(s) => s,
        Err(_) => IFID,
    }}
}}

/// The iFiction record, read from the embedded block.
#[allow(dead_code)]
pub fn ifiction_xml() -> &'static str {{
    ::core::str::from_utf8(&embedded()[{xml_offset}..]).unwrap_or("")
}}
"#,
        version = env!("CARGO_PKG_VERSION"),
        format = story.identification.format.as_str(),
        title = story.bibliographic.title,
        author = story.bibliographic.author,
        ifid = ifid.as_str(),
        ifid_end = marker_len - 2,
    );
    if wasm_section {
        let _ = write!(
            s,
            r#"
/// The same block in the WASM custom section `ifiction`.
#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code, dead_code)]
#[unsafe(link_section = "ifiction")]
#[used]
static EMBEDDED_SECTION: [u8; {len}] = *include_bytes!({bin});
"#
        );
    }
    s
}

/// `Jane Doe <jane@example.org>` becomes `Jane Doe`.
fn strip_email(author: &str) -> String {
    author
        .split('<')
        .next()
        .unwrap_or(author)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Blank lines separate paragraphs; single line breaks are spaces.
fn paragraphs(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                out.push(current.join(" "));
                current.clear();
            }
        } else {
            current.extend(line.split_whitespace());
        }
    }
    if !current.is_empty() {
        out.push(current.join(" "));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = "1974A053-7DB0-4103-93A1-767C1382C0B7";
    const WEB: &str = "448E73DF-2D2F-47E7-A494-A46B40D4CFB3";

    fn meta() -> Metadata {
        toml::from_str(&format!(
            r#"
            genre = "Mystery"
            description = """
            First line
            continues.

            Second paragraph."""
            [ifid]
            executable = "{EXE}"
            html = "{WEB}"
            "#
        ))
        .unwrap()
    }

    fn package() -> PackageInfo {
        PackageInfo {
            name: "lighthouse".into(),
            authors: vec!["Jane Doe <jane@example.org>".into(), "John Roe".into()],
            description: Some("unused".into()),
            homepage: Some("https://example.org".into()),
            version: "0.1.0".into(),
        }
    }

    #[test]
    fn picks_ifid_per_format() {
        let day = PartialDate::Year(2026);
        let exe =
            build_story(Path::new("."), &meta(), &package(), Format::Executable, day).unwrap();
        let web = build_story(Path::new("."), &meta(), &package(), Format::Html, day).unwrap();
        assert_eq!(exe.story.primary_ifid().unwrap().as_str(), EXE);
        assert_eq!(web.story.primary_ifid().unwrap().as_str(), WEB);
        assert_eq!(web.story.identification.format, Format::Html);
        let err = build_story(Path::new("."), &meta(), &package(), Format::Glulx, day).unwrap_err();
        assert!(matches!(err, Error::MissingIfid { .. }));
    }

    #[test]
    fn lowercase_ifids_become_capitals() {
        let mut m = meta();
        m.ifid
            .insert("html".into(), format!(" {} ", WEB.to_lowercase()));
        let built = build_story(
            Path::new("."),
            &m,
            &package(),
            Format::Html,
            PartialDate::Year(2026),
        )
        .unwrap();
        assert_eq!(built.story.primary_ifid().unwrap().as_str(), WEB);
    }

    #[test]
    fn defaults_from_package() {
        let built = build_story(
            Path::new("."),
            &meta(),
            &package(),
            Format::Executable,
            PartialDate::Year(2026),
        )
        .unwrap();
        let b = &built.story.bibliographic;
        assert_eq!(b.title, "lighthouse");
        assert_eq!(b.author, "Jane Doe and John Roe");
        assert_eq!(
            b.description.as_deref(),
            Some("First line continues.\nSecond paragraph.")
        );
        assert_eq!(
            built.story.contacts.unwrap().url.as_deref(),
            Some("https://example.org")
        );
    }

    #[test]
    fn lint_errors_fail() {
        let mut m = meta();
        m.language = Some("English".into());
        let err = build_story(
            Path::new("."),
            &m,
            &package(),
            Format::Html,
            PartialDate::Year(2026),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Lint(_)), "{err}");
    }

    #[test]
    fn build_info() {
        assert_eq!(
            parse_rustc_version("rustc 1.99.1 (58cf8f9c3 2026-10-13)\n").as_deref(),
            Some("1.99.1")
        );
        assert_eq!(parse_rustc_version("cargo 1.99.1"), None);

        let mut built = build_story(
            Path::new("."),
            &meta(),
            &package(),
            Format::Html,
            PartialDate::Year(2026),
        )
        .unwrap();
        add_build_info(
            &mut built.story,
            &BuildInfo {
                target: Some("wasm32-unknown-unknown".into()),
                rustc_version: Some("1.99.1".into()),
            },
        );
        let release = built
            .story
            .releases
            .as_ref()
            .unwrap()
            .attached
            .as_ref()
            .unwrap();
        assert_eq!(release.compiler.as_deref(), Some("rustc"));
        assert_eq!(release.compiler_version.as_deref(), Some("1.99.1"));
        let section = built.story.format_section.as_ref().unwrap();
        assert_eq!(section.name, "html");
        assert_eq!(
            section.entries,
            [("target".to_owned(), "wasm32-unknown-unknown".to_owned())]
        );
        assert!(!lint::has_errors(&lint::check(&built.story)));
    }

    #[test]
    fn target_formats() {
        assert_eq!(format_for_target("wasm32", "unknown"), Format::Html);
        assert_eq!(format_for_target("wasm32", "wasi"), Format::Executable);
        assert_eq!(format_for_target("x86_64", "linux"), Format::Executable);
    }

    #[test]
    fn unknown_keys_are_errors() {
        assert!(toml::from_str::<Metadata>("titel = \"x\"").is_err());
    }
}
