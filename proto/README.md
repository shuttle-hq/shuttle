## Shuttle-proto

This crate contains the protofiles used to generate the provisioner and runtime services,
as well as the generated code.

If you need to make changes to the protofiles, you need to run the tests for this crate to 
regenerate the generated files in `proto/src/generated`. We do it like this so users don't 
need protoc installed to install `cargo-shuttle`.

To run the tests and generate the files, simply run:

```bash
cargo test -p shuttle-proto
```
