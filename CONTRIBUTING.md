# Contributing

## Raise an Issue

Raising [issues](https://github.com/shuttle-hq/shuttle/issues) is encouraged. We have some templates to help you get started.

## Running Locally

```bash
# clone the repo
git clone git@github.com:shuttle-hq/shuttle.git

# cd into the repo
cd shuttle

# start shuttle service in the background
docker-compose up -d

# login to shuttle service
cargo run --bin cargo-shuttle -- login --api-key "ci-test"

# cd into one of the examples
cd examples/rocket/hello-world/

# run deploy the example
cargo run --bin cargo-shuttle --manifest-path ../../../Cargo.toml -- deploy 

# test if example is working
# (use Host header to specify domain of the service)
curl --header "Host: hello-world-rocket-app.shuttleapp.rs" localhost:8000/hello 
```

## Running Tests

shuttle has reasonable test coverage - and we are working on improving this
every day. We encourage PRs to come with tests. If you're not sure about
what a test should look like, feel free to get in touch.

To run the test suite - just run `cargo test --all-features -- --nocapture` at the root of the repository.

## Committing

We use the [Angular Commit Guidelines](https://github.com/angular/angular/blob/master/CONTRIBUTING.md#commit). We expect all commits to conform to these guidelines.

Furthermore, commits should be squashed before being merged to master.

Also, make sure your commits don't trigger any warnings from Clippy by running: `cargo clippy --tests --all-targets --all-features`. If you have a good reason to contradict Clippy, insert an #allow[] macro, so that it won't complain.
