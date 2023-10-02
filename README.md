<!-- markdownlint-disable -->
<p align="center">
<img width="300" src="https://raw.githubusercontent.com/shuttle-hq/shuttle/master/assets/logo-rectangle-transparent.png"/>
</p>
<br>
<p align="center">
  <a href="https://github.com/shuttle-hq/shuttle/search?l=rust">
    <img alt="language" src="https://img.shields.io/badge/language-Rust-orange.svg">
  </a>
  <a href="https://docs.shuttle.rs/">
    <img alt="docs" src="https://img.shields.io/badge/docs-shuttle.rs-orange">
  </a>
  <a href="https://docs.rs/shuttle-runtime">
    <img alt="crate-docs" src="https://img.shields.io/badge/docs-docs.rs-orange">
  </a>
  <a href="https://status.shuttle.rs/">
    <img alt="status" src="https://img.shields.io/badge/status-blue">
  </a>
  <a href="https://circleci.com/gh/shuttle-hq/shuttle/">
    <img alt="build status" src="https://circleci.com/gh/shuttle-hq/shuttle.svg?style=shield"/>
  </a>
</p>
<p align="center">
  <a href="https://crates.io/crates/cargo-shuttle">
    <img alt="crates" src="https://img.shields.io/crates/d/cargo-shuttle">
  </a>
  <a href="https://discord.gg/shuttle">
    <img alt="discord" src="https://img.shields.io/discord/803236282088161321?logo=discord"/>
  </a>
  <a href="https://twitter.com/shuttle_dev">
    <img alt="Twitter Follow" src="https://img.shields.io/twitter/follow/shuttle_dev">
  </a>
</p>
<p align="center">
  <a href="https://console.algora.io/org/shuttle/bounties?status=open">
    <img alt="open bounties" src="https://img.shields.io/endpoint?url=https%3A%2F%2Fconsole.algora.io%2Fapi%2Fshields%2Fshuttle%2Fbounties%3Fstatus%3Dopen"/>
  </a>
  <a href="https://console.algora.io/org/shuttle/bounties?status=completed">
    <img alt="rewarded bounties" src="https://img.shields.io/endpoint?url=https%3A%2F%2Fconsole.algora.io%2Fapi%2Fshields%2Fshuttle%2Fbounties%3Fstatus%3Dcompleted"/>
  </a>
</p>
<!-- markdownlint-restore -->

---

# Shuttle

[Shuttle](https://www.shuttle.rs/) is a Rust-native cloud development platform that lets you deploy your Rust apps for free.

Shuttle is built for productivity, reliability and performance:

- Zero-Configuration support for Rust using annotations
- Automatic resource provisioning (databases, caches, subdomains, etc.) via [Infrastructure-From-Code](https://www.shuttle.rs/blog/2022/05/09/ifc)
- First-class support for popular Rust frameworks ([Actix Web](https://docs.shuttle.rs/examples/actix), [Rocket](https://docs.shuttle.rs/examples/rocket), [Axum](https://docs.shuttle.rs/examples/axum), and [more](https://docs.shuttle.rs/examples/other))
- Support for deploying Discord bots using [Serenity](https://docs.shuttle.rs/examples/serenity)
- Scalable hosting (with optional self-hosting in the future)

📖 Check out our documentation to get started quickly: [docs.shuttle.rs](https://docs.shuttle.rs)

🙋‍♂️ If you have any questions, join our [Discord](https://discord.gg/shuttle) server.

⭐ If you find Shuttle interesting, and would like to stay up-to-date, consider starring this repo to help spread the word.

![star](https://i.imgur.com/kLWmThm.gif)

## (NEW) Shuttle Console

Your projects can now be viewed on the brand new [Shuttle Console](https://console.shuttle.rs/)!
The CLI is still used for most tasks.

![console-preview](https://i.imgur.com/1qdWipP.gif)
*The GIF above visualizes the ease of adding resources to your project(s), along with how they are displayed in the console.*

## Getting Started

The `cargo-shuttle` CLI can be installed with a pre-built binary or from source with cargo.

Shuttle provides pre-built binaries of the `cargo-shuttle` CLI with every release
for most platforms, they can be found on [our GitHub](https://github.com/shuttle-hq/shuttle/releases/latest).

On Linux and macOS, you can use this install script, which will automatically install the correct target for your OS and distro:

```sh
curl -sSfL https://shuttle.rs/install | bash
```

Our binaries can also be installed using [cargo-binstall](https://github.com/cargo-bins/cargo-binstall).
To install with `cargo-binstall`, run:

```sh
# cargo-binstall can also be installed directly as a binary to skip the compilation time: https://github.com/cargo-bins/cargo-binstall#installation
cargo install cargo-binstall
cargo binstall cargo-shuttle
```

Although a bit slower, you can also install directly with cargo:

```sh
cargo install cargo-shuttle
```

> If installing binstall or cargo-shuttle fails, try adding `--locked` to the install command

After installing, log in with:

```sh
cargo shuttle login
```

To initialize your project, simply write:

```bash
cargo shuttle init --template axum hello-world
# Choose a unique project name!
```

And to deploy it, write:

```bash
cd hello-world
cargo shuttle project start  # Only needed if project has not already been created during init
cargo shuttle deploy --allow-dirty
```

And... that's it!

```text
Service Name:  hello-world
Deployment ID: 3d08ac34-ad63-41c1-836b-99afdc90af9f
Status:        running
Last Updated:  2022-04-01T08:32:34Z
URI:           https://hello-world.shuttleapp.rs
```

Feel free to build on top of the generated `hello-world` boilerplate or take a stab at one of our [examples](https://github.com/shuttle-hq/shuttle-examples).

For the full documentation, visit [our docs](https://docs.shuttle.rs).

## Repositories

| Name | Description |  |  |
|-|-|-|-|
| [shuttle](https://github.com/shuttle-hq/shuttle) 🚀 (This repo) | The core Shuttle product. Contains all crates that users interact with. | [Issues](https://github.com/shuttle-hq/shuttle/issues) | [PRs](https://github.com/shuttle-hq/shuttle/pulls)
| [shuttle-examples](https://github.com/shuttle-hq/shuttle-examples) 👨‍🏫 | Officially maintained examples of projects that can be deployed on Shuttle. Also has a list of [community examples](https://github.com/shuttle-hq/shuttle-examples#community-examples). | [Issues](https://github.com/shuttle-hq/shuttle-examples/issues) | [PRs](https://github.com/shuttle-hq/shuttle-examples/pulls)
| [shuttle-docs](https://github.com/shuttle-hq/shuttle-docs) 📃 | Documentation hosted on [docs.shuttle.rs](https://docs.shuttle.rs/). | [Issues](https://github.com/shuttle-hq/shuttle-docs/issues) | [PRs](https://github.com/shuttle-hq/shuttle-docs/pulls)
| [www](https://github.com/shuttle-hq/www) 🌍 | Our website [shuttle.rs](https://www.shuttle.rs/), including the [blog](https://www.shuttle.rs/blog/tags/all) and [Launchpad newsletter](https://www.shuttle.rs/launchpad). | [Issues](https://github.com/shuttle-hq/www/issues) | [PRs](https://github.com/shuttle-hq/www/pulls)
| [deploy-action](https://github.com/shuttle-hq/deploy-action) ⚙ | GitHub Action for continuous deployments. | [Issues](https://github.com/shuttle-hq/deploy-action/issues) | [PRs](https://github.com/shuttle-hq/deploy-action/pulls)
| [awesome-shuttle](https://github.com/shuttle-hq/awesome-shuttle) 🌟 | An awesome list of Shuttle-hosted projects and resources that users can add to. | [Issues](https://github.com/shuttle-hq/awesome-shuttle/issues) | [PRs](https://github.com/shuttle-hq/awesome-shuttle/pulls)

## Contributing to Shuttle

Contributing to Shuttle is highly encouraged!

Check out our [contributing docs](./CONTRIBUTING.md) and find the appropriate repo above to contribute to.

For development of this repo, check the [development docs](./DEVELOPING.md).

Even if you are not planning to submit any code, joining our [Discord server](https://discord.gg/shuttle) and providing feedback helps us a lot!

### Algora Bounties 💰

To offload work from the engineering team on low-priority issues, we will sometimes add a cash bounty to issues.
Sign up to the [Algora Console](https://console.algora.io/org/shuttle/bounties?status=open) to find open issues with bounties.

## Community and Support

- [GitHub Issues](https://github.com/shuttle-hq/shuttle/issues). Best for: bugs and errors you encounter using Shuttle.
- [Twitter](https://twitter.com/shuttle_dev). Best for: keeping up with announcements, releases, collaborations and other events.
- [Discord](https://discord.gg/shuttle). Best for: *ALL OF THE ABOVE* + help, support, sharing your applications and hanging out with the community.

## Project Status

Check for any outages and incidents on [Shuttle Status](https://status.shuttle.rs/).

We are currently in Public Beta.
Watch "releases" of this repo to get notified of major updates!
Also, check out the [Beta announcement](https://www.shuttle.rs/beta#06) for features we are looking forward to.

- [x] Alpha: We are testing Shuttle, API and deployments may be unstable
- [x] Public Alpha: Anyone can sign up, but go easy on us,
  there are a few kinks
- [x] Public Beta: Stable enough for most non-enterprise use-cases
- [ ] Public: Production-ready!

## Contributors ✨

Thanks goes to these wonderful people:

<!-- markdownlint-disable -->
<a href="https://github.com/shuttle-hq/shuttle/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=shuttle-hq/shuttle" />
</a>

Made with [contrib.rocks](https://contrib.rocks).
