# Shuttle - Deploy Rust apps with a single command

<div style="display: flex; margin-top: 30px; margin-bottom: 30px;">
<img src="https://raw.githubusercontent.com/shuttle-hq/shuttle/main/assets/logo-rectangle-transparent.png" width="400px" style="margin-left: auto; margin-right: auto;"/>
</div>

[Shuttle](https://www.shuttle.dev/) is a Rust-native cloud development platform that lets you deploy your Rust apps for free.

📖 Check out our documentation to get started quickly: [docs.shuttle.dev](https://docs.shuttle.dev)

🙋‍♂️ If you have any questions, join our [Discord](https://discord.gg/shuttle) server.

## Usage

Start by installing the [Shuttle CLI](https://crates.io/crates/cargo-shuttle) by running the following in a terminal ([more installation options](https://docs.shuttle.dev/getting-started/installation)):

```bash
# Linux / macOS
curl -sSfL https://www.shuttle.dev/install | bash

# Windows (Powershell)
iwr https://www.shuttle.dev/install-win | iex
```

Now that Shuttle is installed, you can initialize a project with Axum boilerplate:

```bash
shuttle init --template axum my-axum-app
```

By looking at the `Cargo.toml` file of the generated `my-axum-app` project you will see it has been made to
be a binary crate with a few dependencies including `shuttle-runtime` and `shuttle-axum`.

```toml
axum = "0.8.1"
shuttle-axum = "0.56.0"
shuttle-runtime = "0.56.0"
tokio = "1.28.2"
```

A boilerplate code for your axum project can also be found in `src/main.rs`:

```rust,no_run
use axum::{routing::get, Router};

async fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let router = Router::new().route("/", get(hello_world));

    Ok(router.into())
}
```

Check out [our docs](https://docs.shuttle.dev) to see all the frameworks we support, or
our [examples](https://github.com/shuttle-hq/shuttle-examples) if you prefer that format.

## Running locally

To test your app locally before deploying, use:

```bash
shuttle run
```

You should see your app build and start on the default port 8000. You can test this using;

```bash
curl http://localhost:8000/
# Hello, world!
```

## Deploying

Deploy the service with:

```bash
shuttle deploy
```

Your service will then be made available under a subdomain of `*.shuttle.app`. For example:

```bash
curl https://my-axum-app-0000.shuttle.app/
# Hello, world!
```
