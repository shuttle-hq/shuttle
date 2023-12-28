# shuttle-proto

This crate contains the protofiles used to generate the provisioner and runtime services,
as well as the generated code.
Having the generated files commited means users don't need protoc installed to install `cargo-shuttle`.

If you make changes to the protofiles, run

```bash
./proto/integration.sh generate
```

to regenerate the generated files in `proto/src/generated`.
