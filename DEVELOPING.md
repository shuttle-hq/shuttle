# Developing Shuttle

This document demonstrates how to run the code in this repo, and general tips for developing it.
See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines about commit style, issues, PRs, and more.

---

> 🚨 NOTE 🚨: Big rewrites of Shuttle's infra and backends are ongoing.
> Many parts of this document will be outdated soon.

---

> 🚨 NOTE 🚨: Local development and testing is somewhat limited without a Permit.io API key.

## Project Layout

### Binaries

- `cargo-shuttle` is the CLI used by users to initialize, deploy and manage their projects and services on Shuttle.
- `gateway` starts and manages instances of `deployer`. It proxies commands from the user sent via the CLI on port 8001 and traffic on port 8000 to the correct instance of `deployer`.
- `auth` is an authentication service that creates and manages users. In addition to that, requests to the `gateway` that contain an api-key will be proxied to the `auth` service where it will be converted to a JWT for authorization between internal services (like a `deployer` requesting a database from
`provisioner`).
- `deployer` is a service that runs in its own docker container, one per user project. It manages a project's deployments and state.
- `provisioner` is a service used for requesting databases and other resources, using a gRPC API.
- `admin` is a simple CLI used for admin tasks like reviving and stopping projects, as well as requesting
and renewing SSL certificates through the acme client in the `gateway`.

### Libraries

- `api-client` is a reqwest client for calling the backends.
- `common` contains shared models and functions used by the other libraries and binaries.
- `codegen` contains our proc-macro code which gets exposed to user services from `runtime`.
  The redirect through `runtime` is to make it available under the prettier name of `shuttle_runtime::main`.
- `runtime` contains the `alpha` runtime, which embeds a gRPC server and a `Loader` in a service with the `shuttle_runtime::main` macro.
  The gRPC server receives commands from `deployer` like `start` and `stop`.
  The `Loader` sets up a tracing subscriber and provisions resources for the user service.
- `service` is where our special `Service` trait is defined.
  Anything implementing this `Service` can be loaded by the `deployer` and the local runner in `cargo-shuttle`.
  The `service` library also defines the `ResourceConfigBuilder` trait which is used in our codegen to provision resources.
  The `service` library also contains the utilities we use for compiling user crates with `cargo`.
- `proto` contains the gRPC server and client definitions to allow `deployer` to communicate with `provisioner`, and to allow the `deployer` and `cargo-shuttle` cli to communicate with the `alpha` runtime.
- `resources` contains various implementations of `ResourceBuilder`, which are consumed in the `codegen` to provision resources.
- `services` contains implementations of `Service` for common Rust web frameworks. Anything implementing `Service` can be deployed on Shuttle.

## Running Locally

You can use Docker and docker-compose to test Shuttle locally during development. See the [Docker install](https://docs.docker.com/get-docker/)
and [docker-compose install](https://docs.docker.com/compose/install/) instructions if you do not have them installed already.

> Note for Windows: The current [Makefile](https://github.com/shuttle-hq/shuttle/blob/main/Makefile) does not work on Windows systems by itself - if you want to build the local environment on Windows you could use [Windows Subsystem for Linux](https://learn.microsoft.com/en-us/windows/wsl/install). Additional Windows considerations are listed at the bottom of this page.
> Note for Linux: When building on Linux systems, if the error unknown flag: --build-arg is received, install the docker-buildx package using the package management tool for your particular system.

Clone the Shuttle repository (or your fork):

```bash
git clone git@github.com:shuttle-hq/shuttle.git
cd shuttle
```

> Note: We need the git tags for the local development workflow, but they may not be included when you clone the repository.
To make sure you have them, run `git fetch upstream --tags`, where upstream is the name of the Shuttle remote repository.

The [shuttle examples](https://github.com/shuttle-hq/shuttle-examples) are linked to the main repo as a [git submodule](https://git-scm.com/book/en/v2/Git-Tools-Submodules), to initialize it run the following commands:

```bash
git submodule init
git submodule update
```

You should now be ready to setup a local environment to test code changes to core `shuttle` packages as follows:

### Building images

From the root of the Shuttle repo, build the required images with:

```bash
make images
```

The images get built with [cargo-chef](https://github.com/LukeMathWalker/cargo-chef) and therefore support incremental builds (most of the time).

You can now start a local deployment of Shuttle and the required containers with:

```bash
make up
```

> Note: `make up` can also be run with `SHUTTLE_DETACH=disable`, which means docker-compose will not be run with `--detach`. This is often desirable for local testing.
>
> Note: Other useful commands can be found within the [Makefile](./Makefile).

The API is now accessible on `localhost:8000` (for app proxies) and `localhost:8001` (for the control plane). When running `cargo run -p cargo-shuttle` (in a debug build), the CLI will point itself to `localhost` for its API calls.

### Apply patches

In order to test local changes to the library crates, you may want to add patches to a `.cargo/config.toml` file.
(See [Overriding Dependencies](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html) for more)

The simplest way to generate this file is:

```bash
./scripts/apply-patches
```

See the files [apply-patches.sh](./scripts/apply-patches.sh) and [patches.toml](./scripts/patches.toml) for how it works.

> Note: cargo and rust-analyzer will add `[[patch.unused]]` lines at the bottom of Cargo.lock when patches are applied.
> These should not be included in commits/PRs.
> The easiest way to get rid of them is to comment out all the patch lines in `.cargo/config.toml`, and refresh cargo/r-a.

### Create an admin user + login

Before we can login to our local instance of Shuttle, we need to create a user.
The following script inserts a user into the `auth` state with admin privileges,
and sets and env var to override your default api key with the admin test key.
A shell prompt prefix is added to show that this api key override is active.

```bash
source scripts/local-admin.sh
# If you have already done this before you will get a "UNIQUE constraint failed" error. It can be ignored.
```

Finally, before gateway will be able to work with some projects, we need to create a user for it.
The following command inserts a gateway user into the `auth` state with deployer privileges:

```bash
docker compose -f docker-compose.rendered.yml -p shuttle-dev exec auth /usr/local/bin/shuttle-auth --db-connection-uri=postgres://postgres:postgres@control-db init-deployer --user-id gateway --key gateway4deployes
```

### Deploying locally

Create a new project based on one of the examples.
This will prompt your local gateway to start a deployer container.
Then, deploy it.

```bash
cargo run -p cargo-shuttle -- --wd examples/rocket/hello-world project start
cargo run -p cargo-shuttle -- --wd examples/rocket/hello-world deploy
```

Test if the deployment is working:

```bash
# the Host header should match the URI from the deploy output
curl -H "Host: hello-world-rocket-app.unstable.shuttleapp.rs" localhost:8000
#              ^^^^^^^^^^^^^^^^^^^^^^ this will be the project name
```

View logs from the current deployment:

```bash
# append `--follow` to this command for a live feed of logs
cargo run -p cargo-shuttle -- --wd examples/rocket/hello-world logs
```

### Testing deployer only

The steps outlined above starts all the services used by Shuttle locally (ie. both `gateway` and `deployer`). However, sometimes you will want to quickly test changes to `deployer` only. To do this replace `make up` with the following:

```bash
# if you didn't do this already, make the images
make images

# then generate the local docker-compose file
make docker-compose.rendered.yml

# then run
docker compose -f docker-compose.rendered.yml up provisioner resource-recorder logger otel-collector
```

This starts the provisioner and the auth service, while preventing `gateway` from starting up.
Make sure an admin user is inserted into auth and that the key is used by cargo-shuttle. See above.

We're now ready to start a local run of the deployer:

```bash
OTLP_ADDRESS=http://127.0.0.1:4317 cargo run -p shuttle-deployer -- --provisioner-address http://localhost:3000 --auth-uri http://localhost:8008 --resource-recorder http://localhost:8007 --logger-uri http://localhost:8010 --proxy-fqdn local.rs --admin-secret dh9z58jttoes3qvt --local --project-id "01H7WHDK23XYGSESCBG6XWJ1V0" --project <name>
```

The `<name>` needs to match the name of the project that will be deployed to this deployer.
This is the `Cargo.toml` or `Shuttle.toml` name for the project.

Now that your local deployer is running, you can run commands against it using the cargo-shuttle CLI.
It needs to have the same project name as the one you submitted when starting the deployer above.

```bash
cargo run -p cargo-shuttle -- --wd <path> --name <name> deploy
```

### Docker config

#### Docker Desktop

If using Docker Desktop on Unix, you might find adding this to your shell config useful to make `bollard` find the Docker socket:

```sh
export DOCKER_HOST="unix://$HOME/.docker/desktop/docker.sock"
```

#### Using Podman instead of Docker

If you want to use Podman instead of Docker, you can configure the build process with environment variables.

Use Podman for building container images by setting `DOCKER_BUILD`.

```sh
export DOCKER_BUILD=podman build --network host
```

The Shuttle containers expect access to a Docker-compatible API over a socket. Expose a rootless Podman socket either

- [with systemd](https://github.com/containers/podman/tree/main/contrib/systemd), if your system supports it,

    ```sh
    systemctl start --user podman.service
    ```

- or by [running the server directly](https://docs.podman.io/en/latest/markdown/podman-system-service.1.html).

    ```sh
    podman system service --time=0 unix://$XDG_RUNTIME_DIR/podman.sock
    ```

Then set `DOCKER_SOCK` to the *absolute path* of the socket (no protocol prefix).

```sh
export DOCKER_SOCK=$(podman system info -f "{{.Host.RemoteSocket.Path}}")
```

Finally, configure Docker Compose. You can either

- configure Docker Compose to use the Podman socket by setting `DOCKER_HOST` (including the `unix://` protocol prefix),

    ```sh
    export DOCKER_HOST=unix://$(podman system info -f "{{.Host.RemoteSocket.Path}}")
    ```

- or install [Podman Compose](https://github.com/containers/podman-compose) and use it by setting `DOCKER_COMPOSE`.

    ```sh
    export DOCKER_COMPOSE=podman-compose
    ```

If you are using `nftables`, even with `iptables-nft`, it may be necessary to install and configure the [nftables CNI plugins](https://github.com/greenpau/cni-plugins)

## Testing the Pro tier

We use Stripe to start Pro subscriptions and verify them with a Stripe client that needs a secret key. The `STRIPE_SECRET_KEY` environment variable
should be set to test upgrading a user to Pro tier, or to use a Pro tier feature with cargo-shuttle CLI. On a local environment, that requires
setting up a Stripe account and generating a test API key. Auth can still be initialised and used without a Stripe secret key, but it will fail
when retrieving a user, and when we'll verify the subscription validity.

## Running Tests

Install `cargo-make`.

To run the unit tests for a specific crate, from the root of the repository run:

```bash
# replace <crate-name> with the name of the crate to test, e.g. `shuttle-common`
cargo make test-member <crate-name>
```

Integration tests are split between those that rely on Docker, and those who don't.

To run the integration tests for a specific crate (if it has any), from the root of the repository run:

```bash
cargo make test-member-integration <crate-name>
# tests that depend on Docker
cargo make test-member-integration-docker <crate-name>
```

## Windows Considerations

Currently, if you try to use 'make images' on Windows, you may find that the shell files cannot be read by Bash/WSL. This is due to the fact that Windows may have pulled the files in CRLF format rather than LF[^1], which causes problems with Bash as to run the commands, Linux needs the file in LF format.

Thankfully, we can fix this problem by simply using the `git config core.autocrlf` command to change how Git handles line endings. It takes a single argument:

```bash
git config --global core.autocrlf input
```

This should allow you to run `make images` and other Make commands with no issues.

If you need to change it back for whatever reason, you can just change the last argument from 'input' to 'true' like so:

```bash
git config --global core.autocrlf true
```

After you run this command, you should be able to checkout projects that are maintained using CRLF (Windows) again.

[^1]: <https://git-scm.com/book/en/v2/Customizing-Git-Git-Configuration#_core_autocrlf>
