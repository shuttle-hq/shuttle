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
- First-class support for popular Rust frameworks ([Actix Web](https://docs.shuttle.rs/examples/actix), [Rocket](https://docs.shuttle.rs/examples/rocket), [Axum](https://docs.shuttle.rs/examples/axum),
  [Tide](https://docs.shuttle.rs/examples/tide), [Poem](https://docs.shuttle.rs/examples/poem) and [Tower](https://docs.shuttle.rs/examples/tower))
- Support for deploying Discord bots using [Serenity](https://docs.shuttle.rs/examples/serenity)
- Scalable hosting (with optional self-hosting)

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

Our binaries can also be installed using [cargo-binstall](https://github.com/cargo-bins/cargo-binstall),
which will automatically install the correct target for your system.
To install with `cargo-binstall`, run:

```sh
# cargo-binstall can also be installed directly as a binary to skip the compilation time: https://github.com/cargo-bins/cargo-binstall#installation
cargo install cargo-binstall
cargo binstall cargo-shuttle
# If installing binstall or cargo-shuttle fails, try adding `--locked` to the install command
```

Although a bit slower, you can also install directly with cargo:

```sh
cargo install cargo-shuttle
```

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

<!-- Update list below with: make bump-contributors -->
<!-- markdownlint-disable -->
<!-- CONTRIBUTOR LIST -->
[<img alt="chesedo" src="https://avatars.githubusercontent.com/u/5367103?v=4&s=100" width="100">](https://github.com/chesedo) |[<img alt="oddgrd" src="https://avatars.githubusercontent.com/u/29732646?v=4&s=100" width="100">](https://github.com/oddgrd) |[<img alt="brokad" src="https://avatars.githubusercontent.com/u/13315034?v=4&s=100" width="100">](https://github.com/brokad) |[<img alt="christos-h" src="https://avatars.githubusercontent.com/u/14791384?v=4&s=100" width="100">](https://github.com/christos-h) |[<img alt="jonaro00" src="https://avatars.githubusercontent.com/u/54029719?v=4&s=100" width="100">](https://github.com/jonaro00) |[<img alt="iulianbarbu" src="https://avatars.githubusercontent.com/u/14218860?v=4&s=100" width="100">](https://github.com/iulianbarbu) |
:---: |:---: |:---: |:---: |:---: |:---: |
[chesedo](https://github.com/chesedo) |[oddgrd](https://github.com/oddgrd) |[brokad](https://github.com/brokad) |[christos-h](https://github.com/christos-h) |[jonaro00](https://github.com/jonaro00) |[iulianbarbu](https://github.com/iulianbarbu) |

[<img alt="thecotne" src="https://avatars.githubusercontent.com/u/1606993?v=4&s=100" width="100">](https://github.com/thecotne) |[<img alt="bmoxb" src="https://avatars.githubusercontent.com/u/42641081?v=4&s=100" width="100">](https://github.com/bmoxb) |[<img alt="kaleidawave" src="https://avatars.githubusercontent.com/u/26967284?v=4&s=100" width="100">](https://github.com/kaleidawave) |[<img alt="kierendavies" src="https://avatars.githubusercontent.com/u/878309?v=4&s=100" width="100">](https://github.com/kierendavies) |[<img alt="jmwill86" src="https://avatars.githubusercontent.com/u/19667780?v=4&s=100" width="100">](https://github.com/jmwill86) |[<img alt="iamwacko" src="https://avatars.githubusercontent.com/u/101361189?v=4&s=100" width="100">](https://github.com/iamwacko) |
:---: |:---: |:---: |:---: |:---: |:---: |
[thecotne](https://github.com/thecotne) |[bmoxb](https://github.com/bmoxb) |[kaleidawave](https://github.com/kaleidawave) |[kierendavies](https://github.com/kierendavies) |[jmwill86](https://github.com/jmwill86) |[iamwacko](https://github.com/iamwacko) |

[<img alt="ivancernja" src="https://avatars.githubusercontent.com/u/14149737?v=4&s=100" width="100">](https://github.com/ivancernja) |[<img alt="joshua-mo-143" src="https://avatars.githubusercontent.com/u/102877324?v=4&s=100" width="100">](https://github.com/joshua-mo-143) |[<img alt="gautamprikshit1" src="https://avatars.githubusercontent.com/u/25322232?v=4&s=100" width="100">](https://github.com/gautamprikshit1) |[<img alt="Xavientois" src="https://avatars.githubusercontent.com/u/34867186?v=4&s=100" width="100">](https://github.com/Xavientois) |[<img alt="paulotten" src="https://avatars.githubusercontent.com/u/843079?v=4&s=100" width="100">](https://github.com/paulotten) |[<img alt="akrantz01" src="https://avatars.githubusercontent.com/u/16374390?v=4&s=100" width="100">](https://github.com/akrantz01) |
:---: |:---: |:---: |:---: |:---: |:---: |
[ivancernja](https://github.com/ivancernja) |[joshua-mo-143](https://github.com/joshua-mo-143) |[gautamprikshit1](https://github.com/gautamprikshit1) |[Xavientois](https://github.com/Xavientois) |[paulotten](https://github.com/paulotten) |[akrantz01](https://github.com/akrantz01) |

[<img alt="beyarkay" src="https://avatars.githubusercontent.com/u/33420535?v=4&s=100" width="100">](https://github.com/beyarkay) |[<img alt="marioidival" src="https://avatars.githubusercontent.com/u/1129263?v=4&s=100" width="100">](https://github.com/marioidival) |[<img alt="nahuakang" src="https://avatars.githubusercontent.com/u/18533347?v=4&s=100" width="100">](https://github.com/nahuakang) |[<img alt="orhun" src="https://avatars.githubusercontent.com/u/24392180?v=4&s=100" width="100">](https://github.com/orhun) |[<img alt="coszio" src="https://avatars.githubusercontent.com/u/62079184?v=4&s=100" width="100">](https://github.com/coszio) |[<img alt="AlphaKeks" src="https://avatars.githubusercontent.com/u/85143381?v=4&s=100" width="100">](https://github.com/AlphaKeks) |
:---: |:---: |:---: |:---: |:---: |:---: |
[beyarkay](https://github.com/beyarkay) |[marioidival](https://github.com/marioidival) |[nahuakang](https://github.com/nahuakang) |[orhun](https://github.com/orhun) |[coszio](https://github.com/coszio) |[AlphaKeks](https://github.com/AlphaKeks) |

[<img alt="guerinoni" src="https://avatars.githubusercontent.com/u/41150432?v=4&s=100" width="100">](https://github.com/guerinoni) |[<img alt="hseeberger" src="https://avatars.githubusercontent.com/u/41911?v=4&s=100" width="100">](https://github.com/hseeberger) |[<img alt="Kazy" src="https://avatars.githubusercontent.com/u/59063?v=4&s=100" width="100">](https://github.com/Kazy) |[<img alt="Butch78" src="https://avatars.githubusercontent.com/u/19205392?v=4&s=100" width="100">](https://github.com/Butch78) |[<img alt="trezm" src="https://avatars.githubusercontent.com/u/1271597?v=4&s=100" width="100">](https://github.com/trezm) |[<img alt="imor" src="https://avatars.githubusercontent.com/u/1666073?v=4&s=100" width="100">](https://github.com/imor) |
:---: |:---: |:---: |:---: |:---: |:---: |
[guerinoni](https://github.com/guerinoni) |[hseeberger](https://github.com/hseeberger) |[Kazy](https://github.com/Kazy) |[Butch78](https://github.com/Butch78) |[trezm](https://github.com/trezm) |[imor](https://github.com/imor) |

[<img alt="Procrat" src="https://avatars.githubusercontent.com/u/607786?v=4&s=100" width="100">](https://github.com/Procrat) |[<img alt="SyedFasiuddin" src="https://avatars.githubusercontent.com/u/66054777?v=4&s=100" width="100">](https://github.com/SyedFasiuddin) |[<img alt="morlinbrot" src="https://avatars.githubusercontent.com/u/22527555?v=4&s=100" width="100">](https://github.com/morlinbrot) |[<img alt="jhawkesworth" src="https://avatars.githubusercontent.com/u/8440323?v=4&s=100" width="100">](https://github.com/jhawkesworth) |[<img alt="timonv" src="https://avatars.githubusercontent.com/u/49373?v=4&s=100" width="100">](https://github.com/timonv) |[<img alt="piewhat" src="https://avatars.githubusercontent.com/u/17803752?v=4&s=100" width="100">](https://github.com/piewhat) |
:---: |:---: |:---: |:---: |:---: |:---: |
[Procrat](https://github.com/Procrat) |[SyedFasiuddin](https://github.com/SyedFasiuddin) |[morlinbrot](https://github.com/morlinbrot) |[jhawkesworth](https://github.com/jhawkesworth) |[timonv](https://github.com/timonv) |[piewhat](https://github.com/piewhat) |

[<img alt="Anafabula" src="https://avatars.githubusercontent.com/u/57800226?v=4&s=100" width="100">](https://github.com/Anafabula) |[<img alt="angelorendina" src="https://avatars.githubusercontent.com/u/68086271?v=4&s=100" width="100">](https://github.com/angelorendina) |[<img alt="Antonio-dev1" src="https://avatars.githubusercontent.com/u/87125037?v=4&s=100" width="100">](https://github.com/Antonio-dev1) |[<img alt="arturaz" src="https://avatars.githubusercontent.com/u/12931?v=4&s=100" width="100">](https://github.com/arturaz) |[<img alt="sd2k" src="https://avatars.githubusercontent.com/u/5464991?v=4&s=100" width="100">](https://github.com/sd2k) |[<img alt="canac" src="https://avatars.githubusercontent.com/u/3740187?v=4&s=100" width="100">](https://github.com/canac) |
:---: |:---: |:---: |:---: |:---: |:---: |
[Anafabula](https://github.com/Anafabula) |[angelorendina](https://github.com/angelorendina) |[Antonio-dev1](https://github.com/Antonio-dev1) |[arturaz](https://github.com/arturaz) |[sd2k](https://github.com/sd2k) |[canac](https://github.com/canac) |

[<img alt="SonicZentropy" src="https://avatars.githubusercontent.com/u/12196028?v=4&s=100" width="100">](https://github.com/SonicZentropy) |[<img alt="alsuren" src="https://avatars.githubusercontent.com/u/254647?v=4&s=100" width="100">](https://github.com/alsuren) |[<img alt="d4ckard" src="https://avatars.githubusercontent.com/u/89748807?v=4&s=100" width="100">](https://github.com/d4ckard) |[<img alt="emmakuen" src="https://avatars.githubusercontent.com/u/85257928?v=4&s=100" width="100">](https://github.com/emmakuen) |[<img alt="Fuzzicles" src="https://avatars.githubusercontent.com/u/59322784?v=4&s=100" width="100">](https://github.com/Fuzzicles) |[<img alt="GugaGongadze" src="https://avatars.githubusercontent.com/u/5684735?v=4&s=100" width="100">](https://github.com/GugaGongadze) |
:---: |:---: |:---: |:---: |:---: |:---: |
[SonicZentropy](https://github.com/SonicZentropy) |[alsuren](https://github.com/alsuren) |[d4ckard](https://github.com/d4ckard) |[emmakuen](https://github.com/emmakuen) |[Fuzzicles](https://github.com/Fuzzicles) |[GugaGongadze](https://github.com/GugaGongadze) |

[<img alt="HexPandaa" src="https://avatars.githubusercontent.com/u/47880094?v=4&s=100" width="100">](https://github.com/HexPandaa) |[<img alt="sentinel1909" src="https://avatars.githubusercontent.com/u/40224978?v=4&s=100" width="100">](https://github.com/sentinel1909) |[<img alt="jdrouet" src="https://avatars.githubusercontent.com/u/6329508?v=4&s=100" width="100">](https://github.com/jdrouet) |[<img alt="kianmeng" src="https://avatars.githubusercontent.com/u/134518?v=4&s=100" width="100">](https://github.com/kianmeng) |[<img alt="lilianmoraru" src="https://avatars.githubusercontent.com/u/621738?v=4&s=100" width="100">](https://github.com/lilianmoraru) |[<img alt="Luna2141" src="https://avatars.githubusercontent.com/u/60728436?v=4&s=100" width="100">](https://github.com/Luna2141) |
:---: |:---: |:---: |:---: |:---: |:---: |
[HexPandaa](https://github.com/HexPandaa) |[sentinel1909](https://github.com/sentinel1909) |[jdrouet](https://github.com/jdrouet) |[kianmeng](https://github.com/kianmeng) |[lilianmoraru](https://github.com/lilianmoraru) |[Luna2141](https://github.com/Luna2141) |

[<img alt="biryukovmaxim" src="https://avatars.githubusercontent.com/u/59533214?v=4&s=100" width="100">](https://github.com/biryukovmaxim) |[<img alt="Nereuxofficial" src="https://avatars.githubusercontent.com/u/37740907?v=4&s=100" width="100">](https://github.com/Nereuxofficial) |[<img alt="alekspickle" src="https://avatars.githubusercontent.com/u/22867443?v=4&s=100" width="100">](https://github.com/alekspickle) |[<img alt="robjtede" src="https://avatars.githubusercontent.com/u/3316789?v=4&s=100" width="100">](https://github.com/robjtede) |[<img alt="RobWalt" src="https://avatars.githubusercontent.com/u/26892280?v=4&s=100" width="100">](https://github.com/RobWalt) |[<img alt="MrCoolTheCucumber" src="https://avatars.githubusercontent.com/u/16002713?v=4&s=100" width="100">](https://github.com/MrCoolTheCucumber) |
:---: |:---: |:---: |:---: |:---: |:---: |
[biryukovmaxim](https://github.com/biryukovmaxim) |[Nereuxofficial](https://github.com/Nereuxofficial) |[alekspickle](https://github.com/alekspickle) |[robjtede](https://github.com/robjtede) |[RobWalt](https://github.com/RobWalt) |[MrCoolTheCucumber](https://github.com/MrCoolTheCucumber) |

[<img alt="stavares843" src="https://avatars.githubusercontent.com/u/29093946?v=4&s=100" width="100">](https://github.com/stavares843) |[<img alt="ShouvikGhosh2048" src="https://avatars.githubusercontent.com/u/91585022?v=4&s=100" width="100">](https://github.com/ShouvikGhosh2048) |[<img alt="Shubham8287" src="https://avatars.githubusercontent.com/u/42690084?v=4&s=100" width="100">](https://github.com/Shubham8287) |[<img alt="tguichaoua" src="https://avatars.githubusercontent.com/u/33934311?v=4&s=100" width="100">](https://github.com/tguichaoua) |[<img alt="vroussea" src="https://avatars.githubusercontent.com/u/16578601?v=4&s=100" width="100">](https://github.com/vroussea) |[<img alt="utterstep" src="https://avatars.githubusercontent.com/u/829944?v=4&s=100" width="100">](https://github.com/utterstep) |
:---: |:---: |:---: |:---: |:---: |:---: |
[stavares843](https://github.com/stavares843) |[ShouvikGhosh2048](https://github.com/ShouvikGhosh2048) |[Shubham8287](https://github.com/Shubham8287) |[tguichaoua](https://github.com/tguichaoua) |[vroussea](https://github.com/vroussea) |[utterstep](https://github.com/utterstep) |

[<img alt="XaviFP" src="https://avatars.githubusercontent.com/u/30369208?v=4&s=100" width="100">](https://github.com/XaviFP) |[<img alt="XyLyXyRR" src="https://avatars.githubusercontent.com/u/39663597?v=4&s=100" width="100">](https://github.com/XyLyXyRR) |[<img alt="lecoqjacob" src="https://avatars.githubusercontent.com/u/9278174?v=4&s=100" width="100">](https://github.com/lecoqjacob) |[<img alt="d2weber" src="https://avatars.githubusercontent.com/u/29163905?v=4&s=100" width="100">](https://github.com/d2weber) |[<img alt="figsoda" src="https://avatars.githubusercontent.com/u/40620903?v=4&s=100" width="100">](https://github.com/figsoda) |[<img alt="mikegin" src="https://avatars.githubusercontent.com/u/6836398?v=4&s=100" width="100">](https://github.com/mikegin) |
:---: |:---: |:---: |:---: |:---: |:---: |
[XaviFP](https://github.com/XaviFP) |[XyLyXyRR](https://github.com/XyLyXyRR) |[lecoqjacob](https://github.com/lecoqjacob) |[d2weber](https://github.com/d2weber) |[figsoda](https://github.com/figsoda) |[mikegin](https://github.com/mikegin) |


