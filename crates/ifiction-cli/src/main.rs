//! `ifiction`: Treaty of Babel metadata on the command line.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use ifiction::detect::{self, FileKind, Source};
use ifiction::html::HtmlSource;
use ifiction::lint::{self, Severity};
use ifiction::model::{Format, IfIndex};
use ifiction::{Ifid, html, read, scan, write};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print new UUID-based IFIDs.
    NewIfid {
        /// How many.
        #[arg(short = 'n', long, default_value_t = 1)]
        count: usize,
    },
    /// Add an IFID per format to [package.metadata.ifiction.ifid] in Cargo.toml.
    ///
    /// Keeps IFIDs that are there; only missing formats get new ones.
    Init {
        /// The Cargo.toml, or its directory.
        #[arg(long, default_value = ".")]
        manifest: PathBuf,
        /// The formats that need an IFID.
        #[arg(long, value_delimiter = ',', default_value = "executable,html")]
        formats: Vec<String>,
    },
    /// Print the IFIDs of files, like `babel -ifid`.
    Ifid {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Also say where each IFID came from.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Print the iFiction record of a file, or build it from a Cargo.toml.
    Meta {
        /// An iFiction file, a binary with an embedded record, or a Cargo.toml (or its
        /// directory).
        path: PathBuf,
        /// The format to build for, with a Cargo.toml.
        #[arg(long, default_value = "executable")]
        format: String,
    },
    /// Check an iFiction record against the Treaty, like `babel -lint`.
    Lint {
        /// An iFiction file or a binary with an embedded record.
        file: PathBuf,
        /// Fail on warnings too.
        #[arg(long)]
        strict: bool,
    },
    /// Summarize a file: title, author, IFIDs, format.
    Identify { file: PathBuf },
    /// The IFID meta tag of HTML story files.
    #[command(subcommand)]
    Html(HtmlCommand),
}

#[derive(Subcommand)]
enum HtmlCommand {
    /// Put the IFID meta tag into <head>, or replace the one there.
    Inject {
        /// The HTML file; it is changed in place unless --output is given.
        file: PathBuf,
        /// The IFID to inject.
        #[arg(long, conflicts_with = "manifest")]
        ifid: Option<String>,
        /// Take the `html` IFID from this Cargo.toml (or its directory).
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Write here instead; `-` is standard output.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Print the IFID of an HTML file.
    Extract { file: PathBuf },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::NewIfid { count } => {
            for _ in 0..count {
                println!("{}", Ifid::new_v4());
            }
        }
        Command::Init { manifest, formats } => init(&manifest_path(&manifest), &formats)?,
        Command::Ifid { files, verbose } => {
            for file in files {
                let bytes = read_file(&file)?;
                let d = detect::detect(&bytes);
                for ifid in &d.ifids {
                    if verbose {
                        println!(
                            "IFID: {ifid} ({}, {})",
                            describe_source(d.source),
                            describe_kind(d.kind)
                        );
                    } else {
                        println!("IFID: {ifid}");
                    }
                }
            }
        }
        Command::Meta { path, format } => print!("{}", meta(&path, &format)?),
        Command::Lint { file, strict } => return lint_file(&file, strict),
        Command::Identify { file } => identify(&file)?,
        Command::Html(HtmlCommand::Inject {
            file,
            ifid,
            manifest,
            output,
        }) => {
            let ifid = match (ifid, manifest) {
                (Some(i), _) => Ifid::parse_lenient(&i)?,
                (None, Some(m)) => manifest_ifid(&manifest_path(&m), "html")?,
                (None, None) => manifest_ifid(&manifest_path(Path::new(".")), "html")
                    .context("give --ifid or --manifest")?,
            };
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("cannot read {}", file.display()))?;
            let out = html::inject(&text, &ifid)?;
            match output.as_deref() {
                Some(p) if p == Path::new("-") => print!("{out}"),
                Some(p) => write_file(p, &out)?,
                None => write_file(&file, &out)?,
            }
        }
        Command::Html(HtmlCommand::Extract { file }) => {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("cannot read {}", file.display()))?;
            match html::extract_ifid(&text) {
                Some((ifid, _)) => println!("{ifid}"),
                None => bail!("{} has no IFID", file.display()),
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn manifest_path(p: &Path) -> PathBuf {
    if p.is_dir() {
        p.join("Cargo.toml")
    } else {
        p.to_owned()
    }
}

fn is_manifest(p: &Path) -> bool {
    p.is_dir() || p.file_name().is_some_and(|n| n == "Cargo.toml")
}

fn read_file(p: &Path) -> Result<Vec<u8>> {
    std::fs::read(p).with_context(|| format!("cannot read {}", p.display()))
}

fn write_file(p: &Path, text: &str) -> Result<()> {
    std::fs::write(p, text).with_context(|| format!("cannot write {}", p.display()))
}

fn init(manifest: &Path, formats: &[String]) -> Result<()> {
    let text = std::fs::read_to_string(manifest)
        .with_context(|| format!("cannot read {}", manifest.display()))?;
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .with_context(|| format!("cannot parse {}", manifest.display()))?;
    let package = doc
        .get_mut("package")
        .and_then(toml_edit::Item::as_table_mut)
        .with_context(|| format!("{} has no [package]", manifest.display()))?;
    let metadata = sub_table(package, "metadata", true)?;
    let ifiction = sub_table(metadata, "ifiction", false)?;
    let ifids = sub_table(ifiction, "ifid", false)?;
    let mut added = 0;
    for format in formats {
        if let Some(existing) = ifids.get(format).and_then(toml_edit::Item::as_str) {
            Ifid::parse_lenient(existing).with_context(|| format!("existing IFID for {format}"))?;
            println!("{format}: {existing} (kept)");
            continue;
        }
        let ifid = Ifid::new_v4();
        println!("{format}: {ifid} (new)");
        ifids.insert(format, toml_edit::value(ifid.as_str()));
        added += 1;
    }
    if added > 0 {
        write_file(manifest, &doc.to_string())?;
    }
    Ok(())
}

fn sub_table<'a>(
    parent: &'a mut toml_edit::Table,
    key: &str,
    implicit: bool,
) -> Result<&'a mut toml_edit::Table> {
    let item = parent.entry(key).or_insert_with(|| {
        let mut t = toml_edit::Table::new();
        t.set_implicit(implicit);
        toml_edit::Item::Table(t)
    });
    item.as_table_mut()
        .with_context(|| format!("`{key}` is not a table"))
}

fn manifest_ifid(manifest: &Path, format: &str) -> Result<Ifid> {
    let meta = ifiction_build::read_metadata(manifest)?;
    let text = meta.ifid.get(format).with_context(|| {
        format!(
            "{} has no {format} IFID; run `ifiction init`",
            manifest.display()
        )
    })?;
    Ok(Ifid::parse_lenient(text)?)
}

/// The record of a file, or one built from a manifest.
fn meta(path: &Path, format: &str) -> Result<String> {
    if is_manifest(path) {
        let manifest = manifest_path(path);
        let dir = manifest.parent().unwrap_or(Path::new("."));
        let meta = ifiction_build::read_metadata(&manifest)?;
        let package = ifiction_build::PackageInfo::from_manifest(&manifest)?;
        let built = ifiction_build::build_story(
            dir,
            &meta,
            &package,
            Format::from(format),
            ifiction_build::originated(),
        )?;
        for w in &built.warnings {
            eprintln!("{w}");
        }
        return Ok(write::to_string(&IfIndex::single(built.story)));
    }
    let (mut xml, _) = record_of(path)?;
    if !xml.ends_with('\n') {
        xml.push('\n');
    }
    Ok(xml)
}

/// The iFiction XML of a file: the file itself, or a record embedded in it.
fn record_of(path: &Path) -> Result<(String, IfIndex)> {
    let bytes = read_file(path)?;
    if FileKind::sniff(&bytes) == FileKind::IFiction {
        let xml = String::from_utf8(bytes).context("an iFiction file is UTF-8")?;
        let index = read::from_str(&xml)?;
        return Ok((xml, index));
    }
    let Some((xml, index)) = scan::find_ifiction(&bytes).into_iter().next() else {
        bail!("{} has no iFiction record", path.display());
    };
    Ok((xml.to_owned(), index))
}

fn lint_file(path: &Path, strict: bool) -> Result<ExitCode> {
    let (_, index) = record_of(path)?;
    let diagnostics = lint::check_index(&index);
    for d in &diagnostics {
        eprintln!("{d}");
    }
    let failed = diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error || strict);
    if failed {
        return Ok(ExitCode::FAILURE);
    }
    if let Some(ifid) = index.stories.first().and_then(|s| s.primary_ifid()) {
        println!("Verified {ifid}");
    }
    Ok(ExitCode::SUCCESS)
}

fn identify(path: &Path) -> Result<()> {
    let bytes = read_file(path)?;
    let d = detect::detect(&bytes);
    let record = if d.kind == FileKind::IFiction {
        std::str::from_utf8(&bytes)
            .ok()
            .and_then(|x| read::from_str(x).ok())
    } else {
        scan::find_ifiction(&bytes)
            .into_iter()
            .next()
            .map(|(_, i)| i)
    };
    if let Some(story) = record.as_ref().and_then(|r| r.stories.first()) {
        let b = &story.bibliographic;
        println!("\"{}\" by {}", b.title, b.author);
        println!("Format: {}", story.identification.format);
    }
    for ifid in &d.ifids {
        println!("IFID: {ifid}");
    }
    println!(
        "File: {}, {} KiB, IFID from {}",
        describe_kind(d.kind),
        bytes.len().div_ceil(1024),
        describe_source(d.source)
    );
    Ok(())
}

fn describe_source(s: Source) -> &'static str {
    match s {
        Source::IFiction => "iFiction record",
        Source::Html(HtmlSource::Meta) => "HTML meta tag",
        Source::Html(HtmlSource::Marker) => "HTML UUID marker",
        Source::Html(HtmlSource::Twine) => "Twine story data",
        Source::Marker => "UUID marker",
        Source::Embedded => "embedded iFiction record",
        Source::Fallback => "MD5 fallback",
    }
}

fn describe_kind(k: FileKind) -> &'static str {
    match k {
        FileKind::IFiction => "iFiction",
        FileKind::Html => "HTML",
        FileKind::Elf => "ELF executable",
        FileKind::Mz => "MZ executable",
        FileKind::MachO => "Mach-O executable",
        FileKind::Java => "Java class",
        FileKind::Amiga => "Amiga executable",
        FileKind::Script => "script",
        FileKind::Wasm => "WebAssembly module",
        FileKind::Unknown => "unknown",
    }
}
