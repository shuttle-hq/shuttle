# Overview
This project is meant to run all the end-to-end tests for unveil. Here are some notes to help you in your testing
journey.

## Making changes to unveil-service
The examples pull `unveil-service` from crates.io. Therefore, any changes made to `unveil-service` will not be detected
until they are published to crates.io. A way around this is to use the
[`[patch]`](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#the-patch-section) section in
`Cargo.toml` to use the changed `unveil-service` instead. Create a `.cargo/config.toml` in your
[config folder](https://doc.rust-lang.org/cargo/reference/config.html) with the following content.

``` toml
[patch.crates-io]
unveil-service = { path = "[base]/unveil/service" }
```

Now the tests will run against the changes made in `unveil-service`.