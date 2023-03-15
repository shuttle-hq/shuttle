fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path().unwrap();
    let protoc_include = protoc_bin_vendored::include_path().unwrap();

    std::env::set_var("PROTOC", protoc);
    std::env::set_var("PROTOC_INCLUDE", protoc_include);

    tonic_build::configure().compile(&["./provisioner.proto", "./runtime.proto"], &["./"])?;

    Ok(())
}
