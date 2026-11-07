//! A tiny game that carries its own Treaty of Babel metadata.
//!
//! `hello --ifid` prints the IFID, `hello --ifiction` the whole record.

mod ifiction_meta {
    ifiction::include_generated!();
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--ifid") => println!("{}", ifiction_meta::ifid()),
        Some("--ifiction") => print!("{}", ifiction_meta::ifiction_xml()),
        _ => println!(
            "{} by {} ({} build, IFID {})",
            ifiction_meta::TITLE,
            ifiction_meta::AUTHOR,
            ifiction_meta::FORMAT,
            ifiction_meta::ifid()
        ),
    }
}
