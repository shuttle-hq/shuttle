fn main() {
    let gateway_root = std::fs::canonicalize(format!(
        "{}/../gateway",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    ))
    .unwrap();

    // Set the LD_LIBRARY_PATH to the crate root so the sqlite migrations
    // can find the ulid0.so file which is used for sqlite ulid generation.
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", gateway_root.display());
    println!(
        "cargo:rustc-env=DYLD_FALLBACK_LIBRARY_PATH={}",
        gateway_root.display()
    );
}
