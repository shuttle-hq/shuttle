# Contribution guidelines and conventions

This document lists the contribution guidelines for all of Shuttle's repositories.
For a guide on how to run the Shuttle Core stack (this repo), see [DEVELOPING.md](./DEVELOPING.md).

## Tenets

Our goal with Shuttle open-source maintenance is to foster a thriving, collaborative, and sustainable ecosystem around the project, which allows it to continue to grow and evolve over time.

We will strive to adhere to the following tenets:

1. Encourage collaboration: One of the primary objectives of maintenance is to encourage collaboration among contributors. This can be achieved by creating an atmosphere where people feel safe and encouraged to ask questions on PRs and issues. Contributors should feel comfortable asking for clarification, discussing issues, and proposing solutions without fear of criticism or hostility.
2. Communicate transparently: Another objective of maintenance is to ensure transparent communication about the project's goals, progress, and roadmap. This includes providing regular updates on project status, notifying contributors about relevant changes, and communicating expectations for contributions. This creates an environment where people feel they are in the know of things, which helps them feel invested in the project's success.
3. Recognize contributions: Another important objective of maintenance is to recognize contributors' efforts and contributions. This can be achieved by acknowledging contributions publicly, providing feedback and support, and actively engaging with contributors. This creates an environment where people feel their efforts were helpful and that their contributions are valued.
4. Provide support: Finally, an important objective of maintenance is to provide support to contributors when needed. This includes providing guidance on how to contribute, responding to questions and concerns, and helping contributors resolve issues. This creates an environment where people feel they will get help when needed, which helps build trust and fosters collaboration.

## Raise an Issue

Raising issues is encouraged. Please find the appropriate repository in the [repository list](./README.md).

## Docs

If you found an error in our docs, or you simply want to make them better, contributions to our [docs](https://github.com/shuttle-hq/shuttle-docs)
are always appreciated!

## Committing

We use the [Angular Commit Guidelines](https://github.com/angular/angular/blob/master/CONTRIBUTING.md#commit). We expect all commits to conform to these guidelines.

Before committing in Rust repositories:

- Make sure your commits don't trigger any warnings from Clippy by running: `cargo clippy --tests --all-targets --all-features`. If you have a good reason to contradict Clippy, insert an `#[allow(clippy::<lint>)]` macro, so that it won't complain.
- Make sure your code is correctly formatted: `cargo fmt --all --check`.

## Opening a Pull Request

Before opening a pull request it's a good idea to first open an issue, even if the change is small.
This way you can get feedback and suggestions on the issue, as well as a confirmation from the maintainers that this is something we want to implement.
This also greatly increases the likelihood of the pull request getting merged, and it reduces the chance that multiple contributors start working on the same issue in parallel.

We will squash commits before merging your PR to main. If you do want to squash commits, please do not do so
after the review process has started, the commit history can be useful for reviewers.

## Reviewing

Anyone is welcome to review pull requests and provide feedback on issues, as long as they strive to be constructive,
friendly and respectful of the contributor as well as in line with our [code of conduct](CODE_OF_CONDUCT.md).

We will always strive to review pull requests as soon as we can, but during certain periods we may be too busy to
review every pull request in a timely manner. This does not mean we are not excited about the contribution or that
there is anything wrong with it, we are grateful for every contribution and the time spent on them. If you feel that
your PR has gone under the radar for too long feel free to ping us and we'll try to get back to you with an update.
