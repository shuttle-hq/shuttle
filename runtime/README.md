# How to run

## The easy way
Both the legacy and next examples can be run using the local client:

``` bash
cd path/to/example
cargo run --manifest ../../../Cargo.toml --bin cargo-shuttle -- run
```

When a more fine controlled testing is needed, use the instructions below.

## axum-wasm

Compile the wasm axum router:

```bash
make axum
```

Run the test:

```bash
cargo test axum -- --nocapture

# or, run tests
make test
```

Load and run:

```bash
cargo run -- --axum --provisioner-address http://localhost:5000
```

In another terminal:

``` bash
# load
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "/home/<path to shuttle>/runtime/axum.wasm"}' localhost:6001 runtime.Runtime/Load

# start
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "deployment_id": "MDAwMDAwMDAtMDAwMC0wMDAwLTAwMDAtMDAwMDAwMDAwMDAw"}' localhost:6001 runtime.Runtime/Start

# subscribe to logs (unimplemented)
grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:6001 runtime.Runtime/SubscribeLogs

# stop
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "deployment_id": "MDAwMDAwMDAtMDAwMC0wMDAwLTAwMDAtMDAwMDAwMDAwMDAw"}' localhost:6001 runtime.Runtime/Stop
```

Curl the service:
```bash
curl  localhost:7002/hello

curl  localhost:7002/goodbye
```

## shuttle-legacy

Load and run an .so library that implements `shuttle_service::Service`. 

To test, first start a provisioner from the root directory using:

```bash
docker-compose -f docker-compose.rendered.yml up provisioner
```

Then in another shell, start the runtime using the clap CLI:

```bash
cargo run -- --legacy --provisioner-address http://localhost:5000
```

Or directly (this is the path hardcoded in `deployer::start`):
```bash
# first, make sure the shuttle-runtime binary is built
cargo build
# then
/home/<path to shuttle repo>/target/debug/shuttle-runtime --legacy --provisioner-address http://localhost:5000
```

Pass the path to `deployer::start`
Then in another shell, load a `.so` file and start it up:

``` bash
# load
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "/home/<path to shuttle>/examples/rocket/hello-world/target/debug/libhello_world.so"}' localhost:6001 runtime.Runtime/Load

# run (this deployment id is default uuid encoded as base64)
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "deployment_id": "MDAwMDAwMDAtMDAwMC0wMDAwLTAwMDAtMDAwMDAwMDAwMDAw"}' localhost:6001 runtime.Runtime/Start

# subscribe to logs
grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:6001 runtime.Runtime/SubscribeLogs

# stop (the service started in the legacy runtime can't currently be stopped)
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic"}' localhost:6001 runtime.Runtime/Stop
```

## Running the tests
```bash
$ cd ..; make test
```
