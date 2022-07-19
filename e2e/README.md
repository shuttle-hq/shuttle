# Overview
This crate runs all the end-to-end tests for shuttle. These tests must run against a local dev environment, so you first have to set that up by following [these instructions](https://github.com/shuttle-hq/shuttle/blob/main/CONTRIBUTING.md).

Running all the end-to-end tests may take a long time, so it is recommended to run individual tests shipped as part of each crate in the workspace first.

## Running the tests
Simply do

```bash
$ SHUTTLE_API_KEY=test-key cargo test -- --nocapture
```

the `--nocapture` flag helps with logging errors as they arise instead of in one block at the end.

The server-side logs can be accessed with `docker compose logs`.
