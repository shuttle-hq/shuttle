<p align="center">
<img width="300" src="https://raw.githubusercontent.com/getsynth/shuttle/master/resources/logo-rectangle-transparent.png"/>
</p>
<br>
<p align=center>
  <a href="https://docs.rs/shuttle-service">
    <img alt="docs" src="https://img.shields.io/badge/doc-reference-orange">
  </a>
  <a href="https://github.com/getsynth/shuttle/search?l=rust">
    <img alt="language" src="https://img.shields.io/badge/language-Rust-orange.svg">
  </a>
  <a href="https://github.com/getsynth/shuttle/actions">
    <img alt="build status" src="https://img.shields.io/github/workflow/status/getsynth/shuttle/cargo-test"/>
  </a>
  <a href="https://discord.gg/H33rRDTm3p">
    <img alt="discord" src="https://img.shields.io/discord/803236282088161321?logo=discord"/>
  </a>
</p>

---

# shuttle

[Shuttle](https://www.shuttle.rs/) is a serverless platform for Rust which makes it really easy to 
deploy your web-apps.

Shuttle is built for productivity, reliability and performance:
- Zero-Configuration support for Rust using annotations
- Automatic resource provisioning (databases, caches, subdomains, etc.) via [Infrastructure-From-Code](https://www.shuttle.rs/blog/2022/05/09/ifc)
- First-class support for popular Rust frameworks ([Rocket](https://github.com/shuttle-hq/shuttle/tree/main/examples/rocket/hello-world), [Axum](https://github.com/shuttle-hq/shuttle/tree/main/examples/axum/hello-world), 
  [Tide](https://github.com/shuttle-hq/shuttle/tree/main/examples/tide/hello-world) and [Tower](https://github.com/shuttle-hq/shuttle/tree/main/examples/tower/hello-world))
- Scalable hosting (with optional self-hosting)


## Getting Started

First download the Shuttle cargo extension and login:

```bash
$ cargo install cargo-shuttle
$ cargo shuttle login
$ cargo init --lib hello-world
```

Update your `Cargo.toml`:

```toml
[package]
name = "hello-world"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
rocket = "0.5.0-rc.1"
shuttle-service = { version = "0.3.3", features = ["web-rocket"] }
```


Create your first shuttle app in `lib.rs`:

```rust
#[macro_use]
extern crate rocket;

use rocket::{Build, Rocket};

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn rocket() -> Result<Rocket<Build>,shuttle_service::Error> {
    let rocket = rocket::build().mount("/hello", routes![index]);

    Ok(rocket)
}
```

Deploy:

```bash
$ cargo shuttle deploy
   Finished dev [unoptimized + debuginfo] target(s) in 1m 01s

        Project:            hello-world
        Deployment Id:      3d08ac34-ad63-41c1-836b-99afdc90af9f
        Deployment Status:  DEPLOYED
        Host:               hello-world.shuttleapp.rs
        Created At:         2022-04-01 08:32:34.412602556 UTC
        Database URI:       postgres://***:***@pg.shuttle.rs/db-hello-world
```

For the full documentation, visit [docs.rs/shuttle-service](https://docs.rs/shuttle-service)

## Roadmap

For a comprehensive view of the shuttle roadmap check out this [project board](https://github.com/orgs/shuttle-hq/projects/4).

If you have any requests or suggestions feel free to open an issue.

## Community & Support

- [Community Forum](https://github.com/getsynth/shuttle/discussions). Best for: help with building, discussion about best practices.
- [GitHub Issues](https://github.com/getsynth/shuttle/issues). Best for: bugs and errors you encounter using Shuttle.
- [Discord](https://discord.gg/H33rRDTm3p). Best for: sharing your applications and hanging out with the community.
- [Twitter](https://twitter.com/shuttle_dev). Best for: keeping up with announcements and releases

## Status

- [x] Alpha: We are testing Shuttle, API and deployments may be unstable
- [x] Public Alpha: Anyone can sign up, but go easy on us, 
  there are a few kinks
- [ ] Public Beta: Stable enough for most non-enterprise use-cases
- [ ] Public: Production-ready!

We are currently in Public Alpha. Watch "releases" of this repo to get 
notified of major updates!


