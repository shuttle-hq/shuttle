# Contributing

## Raise an Issue

Raising [issues](https://github.com/shuttle-hq/shuttle/issues) is encouraged. We have some templates to help you get started.

## Docs

If you found an error in our docs, or you simply want to make them better, contributions to our [docs](https://github.com/shuttle-hq/shuttle-docs)
are always appreciated!

## Running Locally
You can use Docker and docker-compose to test shuttle locally during development. See the [Docker install](https://docs.docker.com/get-docker/)
and [docker-compose install](https://docs.docker.com/compose/install/) instructions if you do not have them installed already.

You should now be ready to setup a local environment to test code changes to core `shuttle` packages as follows:

Build the required images with:

```bash
make images
```

> Note: The current [Makefile](https://github.com/shuttle-hq/shuttle/blob/main/Makefile) does not work on Windows systems, if you want to build the local environment on Windows you could use [Windows Subsystem for Linux](https://learn.microsoft.com/en-us/windows/wsl/install).

The images get built with [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) and therefore support incremental builds (most of the time). So they will be much faster to re-build after an incremental change in your code - should you wish to deploy it locally straight away.

You can now start a local deployment of shuttle and the required containers with:

```bash
make up
```

> Note: Other useful commands can be found within the [Makefile](https://github.com/shuttle-hq/shuttle/blob/main/Makefile).

The API is now accessible on `localhost:8000` (for app proxies) and `localhost:8001` (for the control plane). When running `cargo run --bin cargo-shuttle` (in a debug build), the CLI will point itself to `localhost` for its API calls.

In order to test local changes to the `shuttle-service` crate, you may want to add the below to a `.cargo/config.toml` file. (See [Overriding Dependencies](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html) for more)

``` toml
[patch.crates-io]
shuttle-service = { path = "[base]/shuttle/service" }
shuttle-aws-rds = { path = "[base]/shuttle/resources/aws-rds" }
shuttle-persist = { path = "[base]/shuttle/resources/persist" }
shuttle-shared-db = { path = "[base]/shuttle/resources/shared-db" }
shuttle-secrets = { path = "[base]/shuttle/resources/secrets" }
```

Prime gateway database with an admin user:

```bash
docker compose --file docker-compose.rendered.yml --project-name shuttle-dev exec gateway /usr/local/bin/service --state=/var/lib/shuttle/gateway.sqlite init --name admin --key test-key
```

Login to shuttle service in a new terminal window from the main shuttle directory:

```bash
cargo run --bin cargo-shuttle -- login --api-key "test-key"
```

cd into one of the examples:

```bash
cd examples/rocket/hello-world/
```

Create a new project, this will start a deployer container:

```bash
# the --manifest-path is used to locate the root of the shuttle workspace
cargo run --manifest-path ../../../Cargo.toml --bin cargo-shuttle -- project new
```

Verify that the deployer is healthy and in the ready state:

```bash
cargo run --manifest-path ../../../Cargo.toml --bin cargo-shuttle -- project status
```

Deploy the example:

```bash
cargo run --manifest-path ../../../Cargo.toml --bin cargo-shuttle -- deploy
```

Test if the deploy is working:

```bash
# the Host header should match the Host from the deploy output
curl --header "Host: {app}.unstable.shuttleapp.rs" localhost:8000/hello
```

View logs from the current deployment:

```bash
# append `--follow` to this command for a live feed of logs
cargo run --manifest-path ../../../Cargo.toml --bin cargo-shuttle -- logs
```

### Testing deployer only
The steps outlined above starts all the services used by shuttle locally (ie. both `gateway` and `deployer`). However, sometimes you will want to quickly test changes to `deployer` only. To do this replace `make up` with the following:

```bash
docker-compose -f docker-compose.rendered.yml up provisioner
```

This prevents `gateway` from starting up. Now you can start deployer only using:

```bash
provisioner_address=$(docker inspect --format '{{(index .NetworkSettings.Networks "shuttle_default").IPAddress}}' shuttle_prod_hello-world-rocket-app_run)
cargo run -p shuttle-deployer -- --provisioner-address $provisioner_address --provisioner-port 8000 --proxy-fqdn local.rs --admin-secret test-key
```

The `--admin-secret` can safely be changed to your api-key to make testing easier.

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

> Note: Testing the `gateway` with a rootless Podman does not work since Podman does not allow access to the `deployer` containers via IP address!

## Running Tests

shuttle has reasonable test coverage - and we are working on improving this
every day. We encourage PRs to come with tests. If you're not sure about
what a test should look like, feel free to [get in touch](https://discord.gg/H33rRDTm3p).

To run the unit tests for a spesific crate, from the root of the repository run:

```bash
# replace <crate-name> with the name of the crate to test, e.g. `shuttle-common`
cargo test --package <crate-name> --all-features --lib -- --nocapture
```

To run the integration tests for a spesific crate (if it has any), from the root of the repository run:

```bash
# replace <crate-name> with the name of the crate to test, e.g. `cargo-shuttle`
cargo test --package <crate-name> --all-features --test '*' -- --nocapture
```

To run the end-to-end tests, from the root of the repository run:

```bash
make test
```

> Note: Running all the end-to-end tests may take a long time, so it is recommended to run individual tests shipped as part of each crate in the workspace first.
## Committing

We use the [Angular Commit Guidelines](https://github.com/angular/angular/blob/master/CONTRIBUTING.md#commit). We expect all commits to conform to these guidelines.

Furthermore, commits should be squashed before being merged to master.

Before committing:
- Make sure your commits don't trigger any warnings from Clippy by running: `cargo clippy --tests --all-targets`. If you have a good reason to contradict Clippy, insert an `#[allow(clippy::<lint>)]` macro, so that it won't complain.
- Make sure your code is correctly formatted: `cargo fmt --all --check`.
- Make sure your `Cargo.toml`'s are sorted: `cargo sort --workspace`. This command uses the [cargo-sort crate](https://crates.io/crates/cargo-sort) to sort the `Cargo.toml` dependencies alphabetically.
- If you've made changes to examples, make sure the above commands are ran there as well.

## Project Layout
The folders in this repository relate to each other as follow:

```mermaid
graph BT
    classDef default fill:#1f1f1f,stroke-width:0,color:white;
    classDef binary fill:#f25100,font-weight:bolder,stroke-width:0,color:white;
    classDef external fill:#343434,font-style:italic,stroke:#f25100,color:white;

    deployer:::binary
    cargo-shuttle:::binary
    common
    codegen
    e2e
    proto
    provisioner:::binary
    service
    gateway:::binary
    user([user service]):::external
    gateway --> common
    gateway -.->|starts instances| deployer
    deployer --> proto
    deployer -.->|calls| provisioner
    service ---> common
    deployer --> common
    cargo-shuttle --->|"features = ['loader']"| service
    deployer -->|"features = ['loader']"| service
    cargo-shuttle --> common
    service --> codegen
    proto ---> common
    provisioner --> proto
    e2e -.->|starts up| gateway
    e2e -.->|calls| cargo-shuttle
    user -->|"features = ['codegen']"| service
```

First, `provisioner`, `gateway`, `deployer`, and `cargo-shuttle` are binary crates with `provisioner`, `gateway` and `deployer` being backend services. The `cargo-shuttle` binary is the `cargo shuttle` command used by users.

The rest are the following libraries:
- `common` contains shared models and functions used by the other libraries and binaries.
- `codegen` contains our proc-macro code which gets exposed to user services from `service` by the `codegen` feature flag. The redirect through `service` is to make it available under the prettier name of `shuttle_service::main`.
- `service` is where our special `Service` trait is defined. Anything implementing this `Service` can be loaded by the `deployer` and the local runner in `cargo-shuttle`.
   The `codegen` automatically implements the `Service` trait for any user service.
- `proto` contains the gRPC server and client definitions to allow `deployer` to communicate with `provisioner`.
- `e2e` just contains tests which starts up the `deployer` in a container and then deploys services to it using `cargo-shuttle`.

Lastly, the `user service` is not a folder in this repository, but is the user service that will be deployed by `deployer`.
