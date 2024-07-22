# Shuttle - Deploy Rust apps with a single Cargo subcommand

<div style="display: flex; margin-top: 30px; margin-bottom: 30px;">
<img src="https://raw.githubusercontent.com/shuttle-hq/shuttle/main/assets/logo-rectangle-transparent.png" width="400px" style="margin-left: auto; margin-right: auto;"/>
</div>

[Shuttle](https://www.shuttle.rs/) is a Rust-native cloud development platform that lets you deploy your Rust apps for free.

📖 Check out our documentation to get started quickly: [docs.shuttle.rs](https://docs.shuttle.rs)

🙋‍♂️ If you have any questions, join our [Discord](https://discord.gg/shuttle) server.

## Usage

Start by installing the [`cargo shuttle`](https://docs.rs/crate/cargo-shuttle/latest) subcommand by running the following in a terminal:

```bash
cargo install cargo-shuttle
```

Now that Shuttle is installed, you can initialize a project with Axum boilerplate:

```bash
cargo shuttle init --template axum my-axum-app
```

By looking at the `Cargo.toml` file of the generated `my-axum-app` project you will see it has been made to
be a binary crate with a few dependencies including `shuttle-runtime` and `shuttle-axum`.

```toml
axum = "0.7.3"
shuttle-axum = "0.47.0"
shuttle-runtime = "0.47.0"
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

Check out [our docs](https://docs.shuttle.rs/introduction/welcome) to see all the frameworks we support, or
our [examples](https://github.com/shuttle-hq/shuttle-examples) if you prefer that format.

## Running locally

To test your app locally before deploying, use:

```bash
cargo shuttle run
```

You should see your app build and start on the default port 8000. You can test this using;

```bash
curl http://localhost:8000/
# Hello, world!
```

## Deploying

Before you can deploy, you have to create a project. This will start a deployer container for your
project under the hood, ensuring isolation from other users' projects. PS. you don't have to do this
now if you did in in the `cargo shuttle init` flow.

```bash
cargo shuttle project start
```

Then, deploy the service with:

```bash
cargo shuttle deploy
```

Your service will immediately be available at `https://{project_name}.shuttleapp.rs/`. For example:

```bash
curl https://my-axum-app.shuttleapp.rs/
# Hello, world!
```
