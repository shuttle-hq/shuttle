fn main() {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    // Set the LD_LIBRARY_PATH to the crate root so the sqlite migrations
    // can find the ulid0.so file which is used for sqlite ulid generation.
    println!("cargo:rustc-env=LD_LIBRARY_PATH={crate_root}");
}
