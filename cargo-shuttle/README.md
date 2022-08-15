<div align="center">

# cargo-shuttle

<p align=center>
  <a href="https://github.com/shuttle-hq/shuttle/search?l=rust">
    <img alt="language" src="https://img.shields.io/badge/language-Rust-orange.svg">
  </a>
  <a href="https://github.com/shuttle-hq/shuttle/actions">
    <img alt="build status" src="https://img.shields.io/github/workflow/status/getsynth/shuttle/cargo-test"/>
  </a>
  <a href="https://discord.gg/H33rRDTm3p">
    <img alt="discord" src="https://img.shields.io/discord/803236282088161321?logo=discord"/>
  </a>
</p>

`cargo-shuttle` is your commandline tool for deploying web apps on [shuttle](https://www.shuttle.rs/), the stateful serverless web platform for Rust.

**README Sections:** [Installation](#installation) — [Subcommands](#subcommands) — [Development](#development)

</div>

---

`cargo-shuttle` brings [shuttle](https://www.shuttle.rs/), the open source serverless platform for Rust web applications, into your terminal. With a dedicated focus on productivity, reliability, and performance, `cargo-shuttle` makes deploying your code to the cloud as easy as deriving a trait.

---

<a id="installation">
<h1>Installation</h1>
</a>

`cargo-shuttle` is available for macOS, Linux, and Windows. To install the commandline tool, run:

```sh
$ cargo install cargo-shuttle
```

---

<a id="subcommands">
<h1>Subcommands</h1>
</a>

`cargo-shuttle`'s subcommands help you build and deploy web apps from start to finish.

Run `cargo-shuttle --help` to see the basic usage:

```
USAGE:
    cargo-shuttle [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --api-url <api-url>
            Run this command against the api at the supplied url [env: SHUTTLE_API=]

        --name <name>                              Specify the name of the project (overrides crate name)
        --working-directory <working-directory>    Specify the working directory [default: .]

SUBCOMMANDS:
    auth      create user credentials for the shuttle platform
    delete    delete the latest deployment for a shuttle project
    deploy    deploy a shuttle project
    help      Prints this message or the help of the given subcommand(s)
    init      create a new shuttle project
    login     login to the shuttle platform
    logs      view the logs of a shuttle project
    run       run a shuttle project locally
    status    view the status of a shuttle project
```

### Subcommand: `init`

To initialize a shuttle project with boilerplates, run `cargo shuttle init [OPTIONS] [PATH]`. 

Currently, `cargo shuttle init` supports the following frameworks:

- `--axum`: for [axum](https://github.com/tokio-rs/axum) framework
- `--poem`: for [poem](https://github.com/poem-web/poem) framework
- `--rocket`: for [rocket](https://rocket.rs/) framework
- `--tide`: for [tide](https://github.com/http-rs/tide) framework
- `--tower`: for [tower](https://github.com/tower-rs/tower) library

For example, running the following command will initialize a project for [rocket](https://rocket.rs/):

```sh
$ cargo shuttle init --rocket my-rocket-app
```

This should generate the following dependency in `Cargo.toml`:
```toml
shuttle-service = { version = "0.4.2", features = ["web-rocket"] }
```

The following boilerplate code should be generated into `src/lib.rs`:

```rust
#[macro_use]
extern crate rocket;

use shuttle_service::ShuttleRocket;

#[get("/hello")]
fn hello() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn init() -> ShuttleRocket {
    let rocket = rocket::build().mount("/", routes![hello]);

    Ok(rocket)
}
```

### Subcommand: `run`

To run the shuttle project locally, use the following command:

```sh
# Inside your shuttle project
$ cargo shuttle run
```

This will compile your shuttle project and start it on the default port `8000`. Test it by:

```sh
$ curl http://localhost:8000/hello
Hello, world!
```

### Subcommand: `login`

Use `cargo shuttle login` inside your shuttle project to generate an API key for the shuttle platform:

```sh
# Inside a shuttle project
$ cargo shuttle login
```

This should automatically open a browser window with an auto-generated API key for your project. Simply copy-paste the API key back in your terminal or run the following command to complete login:

```sh
$ cargo shuttle login --api-key your-api-key-from-browser
```

### Subcommand: `deploy`

To deploy your shuttle project to the cloud, run:

```sh
$ cargo shuttle deploy
```

Your service will immediately be available at `{crate_name}.shuttleapp.rs`. For instance:

```sh
$ curl https://my-rocket-app.shuttleapp.rs/hello
Hello, world!
```

### Subcommand: `status`

Check the status of your deployed shuttle project with:

```sh
$ cargo shuttle status
```

### Subcommand: `logs`

Check the logs of your deployed shuttle project with:

```sh
$ cargo shuttle logs
```

### Subcommand: `auth`

Run the following to create user credentials for shuttle platform:

```sh
$ cargo shuttle auth your-desired-username
```

### Subcommand: `delete`

Once you are done with a deployment, you can delete it by running:

```sh
$ cargo shuttle delete
```

---

<a id="development">
<h1>Development</h1>
</a>

Thanks for using `cargo-shuttle`! We’re very happy to have you with us!

During our alpha period, API keys are completely free and you can deploy as many services as you want.

Just keep in mind that there may be some kinks that require us to take all deployments down once in a while. In certain circumstances we may also have to delete all the data associated with those deployments.

To contribute to `cargo-shuttle` or stay updated with our development, please [open an issue, discussion or PR on Github](https://github.com/shuttle-hq/shuttle) and [join our Discord](https://discord.gg/H33rRDTm3p)! 🚀
