fn main() {
    if let Err(e) = ifiction_build::generate() {
        panic!("{e}");
    }
}
