# Contributing

## Raise an Issue

Raising [issues](https://github.com/shuttle-hq/shuttle/issues) is encouraged. We have some templates to help you get started.

## Running Locally
You can use Docker and docker-compose to test shuttle locally during development. See the [Docker install](https://docs.docker.com/get-docker/)
and [docker-compose install](https://docs.docker.com/compose/install/) instructions if you do not have them installed already.

You should now be set to run shuttle locally as follow:

```bash
# clone the repo
git clone git@github.com:shuttle-hq/shuttle.git

# cd into the repo
cd shuttle

# start the shuttle services
docker-compose up --build

# login to shuttle service in a new terminal window
cd path/to/shuttle/repo
cargo run --bin cargo-shuttle -- login --api-key "ci-test"

# cd into one of the examples
cd examples/rocket/hello-world/

# deploy the example
# the --manifest-path is used to locate the root of the shuttle workspace
cargo run --manifest-path ../../../Cargo.toml --bin cargo-shuttle -- deploy

# test if the deploy is working
# (the Host header should match the Host from the deploy output)
curl --header "Host: hello-world-rocket-app.teste.rs" localhost:8000/hello
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
