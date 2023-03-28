# e2e

## Overview

This crate runs all the end-to-end tests for shuttle. These tests must run against a local dev environment, so you first have to set that up by following [these instructions](../CONTRIBUTING.md).

Running all the end-to-end tests may take a long time, so it is recommended to run individual tests shipped as part of each crate in the workspace first.

## Running the tests

In the root of the repository, run:

```bash
make test
```

To run individual tests, in the root of the e2e directory run:

```bash
POSTGRES_PASSWORD=postgres APPS_FQDN=unstable.shuttleapp.rs cargo test <test name> -- --nocapture
```

The server-side logs can be accessed with `docker compose logs`.
