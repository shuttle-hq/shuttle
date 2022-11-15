# How to run

## shuttle-next
```bash
$ make wasm
$ DISCORD_TOKEN=xxx cargo run
```

In another terminal:

``` bash
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "runtime/axum.wasm"}' localhost:6001 runtime.Runtime/Load
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic"}' localhost:6001 runtime.Runtime/Start
grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:6001 runtime.Runtime/SubscribeLogs
```

## axum-wasm

Compile the wasm axum router:

```bash
make axum
```

Run the test:

```bash
cargo test --all-features axum -- --nocapture
```

Load and run:

```bash
make axum
```

In another terminal:

``` bash
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "runtime/axum.wasm"}' localhost:8000 runtime.Runtime/Load
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic"}' localhost:8000 runtime.Runtime/Start
# grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:8000 runtime.Runtime/SubscribeLogs
```

## shuttle-legacy

Load and run an .so library that implements `shuttle_service::Service`. 

To test, first start a provisioner from the root directory using:

```bash
docker-compose -f docker-compose.rendered.yml up provisioner
```

Then in another shell, start the runtime using the clap CLI:

```bash
cargo run -- --legacy --provisioner-address http://localhost:8000
```

Or directly (this is the path hardcoded in `deployer::start`):
```bash
# first, make sure the shuttle-runtime binary is built
cargo build
# then
/home/<path to shuttle repo>/target/debug/shuttle-runtime --legacy --provisioner-address http://localhost:8000
```

Pass the path to `deployer::start`
Then in another shell, load a `.so` file and start it up:

``` bash
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "examples/rocket/hello-world/target/debug/libhello_world.so"}' localhost:8000 runtime.Runtime/Load
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic"}' localhost:8000 runtime.Runtime/Start
grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:8000 runtime.Runtime/SubscribeLogs
```

## Running the tests
```bash
$ cd ..; make test
```
