# Contributing

## Raise an Issue

Raising [issues](https://github.com/shuttle-hq/shuttle/issues) is encouraged. We have some templates to help you get started.

## Running Locally
You can use Docker and docker-compose to test shuttle locally during development. See the [Docker install](https://docs.docker.com/get-docker/)
and [docker-compose install](https://docs.docker.com/compose/install/) instructions if you do not have them installed already.

You should now be ready to setup a local environment to test code changes to core `shuttle` packages as follows:

Build the required images with:

```bash
$ docker buildx bake -f docker-bake.hcl provisioner api
```

The images get built with [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) and therefore support incremental builds (most of the time). So they will be much faster to re-build after an incremental change in your code - should you wish to deploy it locally straightaway.

Create a docker persistent volume with:

```bash
$ docker volume create shuttle-backend-vol
```

Finally, you can start a local deployment of shuttle with:

```bash
$ docker compose -f docker-compose.dev.yml up -d
```

The API is now accessible on `localhost:8000` (for app proxies) and `localhost:8001` (for the control plane). When running `cargo run --bin cargo-shuttle` (in a debug build), the CLI will point itself to `localhost` for its API calls. The deployment parameters can be tweaked by changing values in the [.env](./.env) file.

In order to test local changes to the `shuttle-service` crate, you may want to add the below to a `.cargo/config.toml` file. (See [Overriding Dependencies](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html) for more)

``` toml
[patch.crates-io]
shuttle-service = { path = "[base]/shuttle/service" }
```

Login to shuttle service in a new terminal window from the main shuttle directory:

```bash
cargo run --bin cargo-shuttle -- login --api-key "test-key"
```

cd into one of the examples:

```bash
cd examples/rocket/hello-world/
```

Deploy the example:

```bash
# the --manifest-path is used to locate the root of the shuttle workspace
cargo run --manifest-path ../../../Cargo.toml --bin cargo-shuttle -- deploy
```

Test if the deploy is working:

```bash
# (the Host header should match the Host from the deploy output)
curl --header "Host: {app}.localhost.local" localhost:8000/hello
```
### Using Podman instead of Docker
If you are using Podman over Docker, then expose a rootless socket of Podman using the following command:

```bash
podman system service --time=0 unix:///tmp/podman.sock
```

Now make docker-compose use this socket by setting the following environment variable:

```bash
export DOCKER_HOST=unix:///tmp/podman.sock
```

shuttle can now be run locally using the steps shown earlier.

## Running Tests

shuttle has reasonable test coverage - and we are working on improving this
every day. We encourage PRs to come with tests. If you're not sure about
what a test should look like, feel free to [get in touch](https://discord.gg/H33rRDTm3p).

To run the test suite - just run `cargo test -- --nocapture` at the root of the repository.

## Committing

We use the [Angular Commit Guidelines](https://github.com/angular/angular/blob/master/CONTRIBUTING.md#commit). We expect all commits to conform to these guidelines.

Furthermore, commits should be squashed before being merged to master.

Also, make sure your commits don't trigger any warnings from Clippy by running: `cargo clippy --tests --all-targets`. If you have a good reason to contradict Clippy, insert an #allow[] macro, so that it won't complain.

## Project Layout
The folders in this repository relate to each other as follow:

```mermaid
graph BT
    classDef default fill:#1f1f1f,stroke-width:0;
    classDef binary fill:#f25100,font-weight:bold,stroke-width:0;
    classDef external fill:#343434,font-style:italic,stroke:#f25100;

    api:::binary
    cargo-shuttle:::binary
    common
    codegen
    e2e
    proto
    provisioner:::binary
    service
    user([user service]):::external
    api --> proto
    api -.->|calls| provisioner
    service ---> common
    api --> common
    cargo-shuttle --->|"features = ['loader']"| service
    api -->|"features = ['loader', 'secrets']"| service
    cargo-shuttle --> common
    service --> codegen
    proto ---> common
    provisioner --> proto
    e2e -.->|starts up| api
    e2e -.->|calls| cargo-shuttle
    user -->|"features = ['codegen']"| service
```

First, `provisioner`, `api`, and `cargo-shuttle` are binary crates with `provisioner` and `api` being backend services. The `cargo-shuttle` binary is the `cargo shuttle` command used by users.

The rest are the following libraries:
- `common` contains shared models and functions used by the other libraries and binaries.
- `codegen` contains our proc-macro code which gets exposed to user services from `service` by the `codegen` feature flag. The redirect through `service` is to make it available under the prettier name of `shuttle_service::main`.
- `service` is where our special `Service` trait is defined. Anything implementing this `Service` can be loaded by the `api` and the local runner in `cargo-shuttle`.
   The `codegen` automatically implements the `Service` trait for any user service.
- `proto` contains the gRPC server and client definitions to allow `api` to communicate with `provisioner`.
- `e2e` just contains tests which starts up the `api` in a container and then deploys services to it using `cargo-shuttle`.

Lastly, the `user service` is not a folder in this repository, but is the user service that will be deployed by `api`.
