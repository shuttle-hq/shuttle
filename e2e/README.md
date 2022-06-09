# Overview
This project is meant to run all the end-to-end tests for shuttle. Here are some notes to help you in your testing
journey.

## Making changes to shuttle-service
The examples pull `shuttle-service` from crates.io. Therefore, any changes made to `shuttle-service` will not be detected
until they are published to crates.io. A way around this is to use the
[`[patch]`](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section) section in
`Cargo.toml` to use the changed `shuttle-service` instead. Create a `.cargo/config.toml` in your
[config folder](https://doc.rust-lang.org/cargo/reference/config.html) with the following content.

``` toml
[patch.crates-io]
shuttle-service = { path = "[base]/shuttle/service" }
```

Now the tests will run against the changes made in `shuttle-service`.
