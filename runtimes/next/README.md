# `shuttle-next`

## How to run

```bash
$ cd ..; make wasm
$ DISCORD_TOKEN=xxx cargo run
```

In another terminal:

``` bash
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic", "path": "bot.wasm"}' localhost:8000 runtime.Runtime/load
grpcurl -plaintext -import-path ../proto -proto runtime.proto -d '{"service_name": "Tonic"}' localhost:8000 runtime.Runtime/start
```

## Running the tests
```bash
$ cd ..; make test
```
