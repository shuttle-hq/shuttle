fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile(
        &["../proto/provisioner.proto", "../proto/runtime.proto"],
        &["../proto"],
    )?;

    Ok(())
}
