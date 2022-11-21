# How to run

## serenity-wasm

Build and run:

## shuttle-next
```bash
$ make wasm
$ DISCORD_TOKEN=xxx cargo run
```

In another terminal:

``` bash
# load wasm module
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "runtime/bot.wasm"}' localhost:6001 runtime.Runtime/Load

# start bot (the deployment ID is needed in the StartRequest, but it isn't used by the serenity bot currently)
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "deployment_id": "MDAwMDAwMDAtMDAwMC0wMDAwLTAwMDAtMDAwMDAwMDAwMDAw"}' localhost:6001 runtime.Runtime/Start

# subscribe to logs (unimplemented)
grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:6001 runtime.Runtime/SubscribeLogs

# stop (the deployment ID is needed in the StopRequest, but it isn't used by the serenity bot currently)
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "deployment_id": "MDAwMDAwMDAtMDAwMC0wMDAwLTAwMDAtMDAwMDAwMDAwMDAw"}' localhost:6001 runtime.Runtime/Stop
```

## axum-wasm

Compile the wasm axum router:

```bash
make axum
```

Run the test:

```bash
cargo test axum --features shuttle-axum -- --nocapture
```

Load and run:

```bash
cargo run --features shuttle-axum -- --axum --provisioner-address http://localhost:8000
```

In another terminal:

``` bash
# load (a full, absolute path from home was needed for me in the load request)
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "runtime/axum.wasm"}' localhost:6001 runtime.Runtime/Load

# start
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic"}' localhost:6001 runtime.Runtime/Start

# subscribe to logs (unimplemented)
grpcurl -plaintext -import-path ../proto -proto runtime.proto localhost:6001 runtime.Runtime/SubscribeLogs
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
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "examples/rocket/hello-world/target/debug/libhello_world.so"}' localhost:6001 runtime.Runtime/Load

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
