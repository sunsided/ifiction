//! Treaty of Babel metadata for interactive fiction.
//!
//! - [`Ifid`]: parse, validate and generate IFIDs, and write `UUID://…//` markers.
//! - [`model`]: the iFiction record; [`write`] and [`read`] turn it into XML and back.
//! - [`lint`]: the rules and guidelines of the Treaty, like `babel -verify` and `-lint`.
//! - [`html`]: the `<meta property="ifiction:ifid">` tag of HTML story files.
//! - [`scan`] and [`detect`]: find IFIDs and records in any file, with babel's fallbacks.
//! - [`include_generated!`]: embed what `ifiction-build` generated into a game.
//!
//! A game that only embeds its metadata needs no features:
//! `ifiction = { version = "…", default-features = false }`.
//!
//! The specification is the Treaty of Babel, revision 13:
//! <https://babel.ifarchive.org/babel.html>. IFID detection matches `babel` 0.75, its
//! reference tool.

pub mod cover;
#[cfg(feature = "detect")]
pub mod detect;
pub mod html;
pub mod ifid;
pub mod lint;
pub mod model;
#[cfg(feature = "read")]
pub mod read;
pub mod scan;
pub mod write;

pub use ifid::{Ifid, IfidError};
pub use model::{
    Auxiliary, Bibliographic, Colophon, Contacts, Cover, CoverFormat, Forgiveness, Format,
    FormatSection, Identification, IfIndex, PartialDate, Release, Releases, Story,
};

/// Includes the metadata module that `ifiction-build` wrote to `OUT_DIR`.
///
/// Use it at module level, typically in its own module:
///
/// ```ignore
/// mod ifiction_meta {
///     ifiction::include_generated!();
/// }
///
/// fn main() {
///     // Read the block once: the MSVC linker drops it otherwise.
///     println!("IFID {}", ifiction_meta::ifid());
/// }
/// ```
///
/// The module defines:
/// - `IFID`, `FORMAT`, `TITLE`, `AUTHOR`: string constants;
/// - `ifid()`: the IFID, read from the embedded block (keeps it alive);
/// - `ifiction_xml()`: the whole iFiction record, also from the embedded block;
/// - `EMBEDDED`: the block itself, `UUID://<IFID>//` and a newline, then the XML.
#[macro_export]
macro_rules! include_generated {
    () => {
        include!(concat!(env!("OUT_DIR"), "/ifiction.rs"));
    };
}
