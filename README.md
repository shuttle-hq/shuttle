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
  <a href="https://circleci.com/gh/shuttle-hq/shuttle/">
    <img alt="build status" src="https://circleci.com/gh/shuttle-hq/shuttle.svg?style=shield"/>
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
```

Create your first shuttle app with `rocket` framework:

```bash
$ cargo shuttle init --rocket hello-world
```

Your `Cargo.toml` should look like:

```toml
[package]
name = "hello-world"
version = "0.1.0"
edition = "2021"

[lib]

[dependencies]
shuttle-service = { version = "0.4.2", features = ["web-rocket"] }
rocket = "0.4.11"
```


Your shuttle app in `lib.rs` should look like:

```rust
#[macro_use]
extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[shuttle_service::main]
async fn rocket() -> shuttle_service::ShuttleRocket {
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

## Contributing to shuttle

If you want to setup a local environment to test code changes to core `shuttle` packages, or want to contribute to the project see [CONTRIBUTING.md](./CONTRIBUTING.md)

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

## Contributors ✨

Thanks goes to these wonderful people ([emoji key](https://allcontributors.org/docs/en/emoji-key)):

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tr>
    <td align="center"><a href="https://github.com/chesedo"><img src="https://avatars.githubusercontent.com/u/5367103?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Pieter</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Achesedo" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=chesedo" title="Code">💻</a> <a href="#example-chesedo" title="Examples">💡</a> <a href="#mentoring-chesedo" title="Mentoring">🧑‍🏫</a> <a href="https://github.com/shuttle-hq/shuttle/pulls?q=is%3Apr+reviewed-by%3Achesedo" title="Reviewed Pull Requests">👀</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=chesedo" title="Tests">⚠️</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=chesedo" title="Documentation">📖</a></td>
    <td align="center"><a href="https://github.com/christoshadjiaslanis"><img src="https://avatars.githubusercontent.com/u/14791384?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Christos Hadjiaslanis</b></sub></a><br /><a href="#blog-christoshadjiaslanis" title="Blogposts">📝</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=christoshadjiaslanis" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/pulls?q=is%3Apr+reviewed-by%3Achristoshadjiaslanis" title="Reviewed Pull Requests">👀</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=christoshadjiaslanis" title="Tests">⚠️</a> <a href="#video-christoshadjiaslanis" title="Videos">📹</a> <a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Achristoshadjiaslanis" title="Bug reports">🐛</a></td>
    <td align="center"><a href="https://github.com/thecotne"><img src="https://avatars.githubusercontent.com/u/1606993?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Tsotne Nazarashvili</b></sub></a><br /><a href="#content-thecotne" title="Content">🖋</a></td>
    <td align="center"><a href="https://github.com/brokad"><img src="https://avatars.githubusercontent.com/u/13315034?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Damien</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Abrokad" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=brokad" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=brokad" title="Documentation">📖</a> <a href="#fundingFinding-brokad" title="Funding Finding">🔍</a> <a href="https://github.com/shuttle-hq/shuttle/pulls?q=is%3Apr+reviewed-by%3Abrokad" title="Reviewed Pull Requests">👀</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=brokad" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/bmoxb"><img src="https://avatars.githubusercontent.com/u/42641081?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Max</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Abmoxb" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=bmoxb" title="Code">💻</a> <a href="#example-bmoxb" title="Examples">💡</a> <a href="https://github.com/shuttle-hq/shuttle/pulls?q=is%3Apr+reviewed-by%3Abmoxb" title="Reviewed Pull Requests">👀</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=bmoxb" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://kaleidawave.github.io/"><img src="https://avatars.githubusercontent.com/u/26967284?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Ben</b></sub></a><br /><a href="#blog-kaleidawave" title="Blogposts">📝</a> <a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Akaleidawave" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=kaleidawave" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=kaleidawave" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/ivancernja"><img src="https://avatars.githubusercontent.com/u/14149737?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Ivan</b></sub></a><br /><a href="#blog-ivancernja" title="Blogposts">📝</a> <a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Aivancernja" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=ivancernja" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=ivancernja" title="Tests">⚠️</a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://github.com/Xavientois"><img src="https://avatars.githubusercontent.com/u/34867186?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Xavientois</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3AXavientois" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=Xavientois" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=Xavientois" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/nahuakang"><img src="https://avatars.githubusercontent.com/u/18533347?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Nahua</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Anahuakang" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=nahuakang" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=nahuakang" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/oddgrd"><img src="https://avatars.githubusercontent.com/u/29732646?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Oddbjørn Grødem</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/commits?author=oddgrd" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=oddgrd" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/marioidival"><img src="https://avatars.githubusercontent.com/u/1129263?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Mario Idival</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/commits?author=marioidival" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=marioidival" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://thesnugco.com/"><img src="https://avatars.githubusercontent.com/u/19667780?v=4?s=100" width="100px;" alt=""/><br /><sub><b>jmwill86</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Ajmwill86" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=jmwill86" title="Documentation">📖</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=jmwill86" title="Tests">⚠️</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=jmwill86" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/coszio"><img src="https://avatars.githubusercontent.com/u/62079184?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Luis Cossío</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/commits?author=coszio" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=coszio" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/SonicZentropy"><img src="https://avatars.githubusercontent.com/u/12196028?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Casey Bailey</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3ASonicZentropy" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=SonicZentropy" title="Documentation">📖</a></td>
  </tr>
  <tr>
    <td align="center"><a href="http://alsuren.github.io/"><img src="https://avatars.githubusercontent.com/u/254647?v=4?s=100" width="100px;" alt=""/><br /><sub><b>David Laban</b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/issues?q=author%3Aalsuren" title="Bug reports">🐛</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=alsuren" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=alsuren" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/lilianmoraru"><img src="https://avatars.githubusercontent.com/u/621738?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Lilian Anatolie Moraru</b></sub></a><br /><a href="#content-lilianmoraru" title="Content">🖋</a></td>
    <td align="center"><a href="https://github.com/Butch78"><img src="https://avatars.githubusercontent.com/u/19205392?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Matthew Aylward </b></sub></a><br /><a href="https://github.com/shuttle-hq/shuttle/commits?author=Butch78" title="Code">💻</a> <a href="https://github.com/shuttle-hq/shuttle/commits?author=Butch78" title="Tests">⚠️</a></td>
    <td align="center"><a href="https://github.com/nodard"><img src="https://avatars.githubusercontent.com/u/12720758?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Nodar Daneliya</b></sub></a><br /><a href="#business-nodard" title="Business development">💼</a> <a href="#fundingFinding-nodard" title="Funding Finding">🔍</a></td>
  </tr>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

This project follows the [all-contributors](https://github.com/all-contributors/all-contributors) specification. Contributions of any kind welcome!