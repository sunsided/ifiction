//! Never reads the embedded block. `scripts/verify-embed.sh` checks that the linker keeps
//! it anyway, through `#[used]` alone.

mod ifiction_meta {
    ifiction::include_generated!();
}

fn main() {
    // A `const`, inlined; it does not refer to `EMBEDDED`.
    println!("{}", ifiction_meta::TITLE);
}
