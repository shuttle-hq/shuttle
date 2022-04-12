# Integration Tests for `cargo-shuttle`

Integration tests are organised following [matklad's recommedations](https://matklad.github.io/2021/02/27/delete-cargo-integration-tests.html).

Initially, everything is tested via [assert_cmd](https://docs.rs/assert_cmd/latest/assert_cmd/cmd/struct.Command.html), but it might make sense to split `cargo-shuttle` into a bin+lib crate, to test the internals more easily.
