# Developing Shuttle

This document demonstrates how to run the code in this repo, and general tips for developing it.
See [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines about commit style, issues, PRs, and more.

---

## Project Layout

### Binaries

- `cargo-shuttle` is the CLI used by users to initialize, run, deploy and manage their projects and services on Shuttle.

### Libraries

- `api-client` is a reqwest client for calling the Shuttle API.
- `common` contains shared models and functions used by the other libraries and binaries.
- `codegen` contains our proc-macro code which gets exposed to user services from `runtime`.
  The redirect through `runtime` is to make it available under the prettier name of `shuttle_runtime::main`.
- `runtime` sets up a tracing subscriber and provisions resources for the user service.
- `service` is where our special `Service` trait is defined.
  Anything implementing `Service` can be started by the `runtime`.
  The `service` library also defines the `ResourceInputBuilder` trait which is used in our codegen to provision resources.
- `resources` contains various implementations of `ResourceInputBuilder` for official resources and plugins.
- `services` contains implementations of `Service` for common Rust frameworks to run on Shuttle. Anything implementing `Service` can be deployed on Shuttle.

## Running Locally

Clone the Shuttle repository (or your fork):

```bash
git clone git@github.com:shuttle-hq/shuttle.git
cd shuttle
```

The [shuttle examples](https://github.com/shuttle-hq/shuttle-examples) are linked to the main repo as a [git submodule](https://git-scm.com/book/en/v2/Git-Tools-Submodules), to initialize it run the following commands:

```bash
git submodule init
git submodule update
```

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

### Testing examples

```bash
cargo run --bin shuttle -- --wd examples/rocket/hello-world run
```

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
