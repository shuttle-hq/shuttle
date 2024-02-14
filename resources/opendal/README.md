# Shuttle OpenDAL

This plugin allows services to connect to [Apache OpenDAL™](https://github.com/apache/opendal). OpenDAL is a data access layer that allows users to easily and efficiently retrieve data from various storage services in a unified way.

Users can connect OpenDAL to access data from a variety of storage services, including: s3, azblob, gcs, oss and [so on](https://opendal.apache.org/docs/rust/opendal/services/index.html).

## Usage

**IMPORTANT**: Currently Shuttle isn't able to provision a storage for you (yet). This means you will have to create the storage service first and setup the secrets accordingly.

Add `shuttle-opendal` to the dependencies for your service by running `cargo add shuttle-opendal`.
This resource will be provided by adding the `shuttle_opendal::Opendal` attribute to your Shuttle `main` decorated function.

It returns a `opendal::Operator` for you to connect the storage service.

### Example

In the case of an Axum server, your main function will look like this:

```rust
use opendal::Operator;
use shuttle_axum::ShuttleAxum;

#[shuttle_runtime::main]
async fn app(
    #[shuttle_opendal::Opendal(scheme = "s3")]
    storage: Operator,
) -> ShuttleAxum {}
```

### Parameters

| Parameter | Type  | Default    | Description                                      |
|-----------|-------|------------|--------------------------------------------------|
| scheme    | `str` | `"memory"` | The scheme of the storage service to connect to. |

All secrets are loaded from your `Secrets.toml` file. 

For instance, when using `s3`, you can configure the scheme to `s3` and specify the secrets: `bucket`, `access_key_id`, and `secret_access_key`.

Visit the [OpenDAL Documentation](https://opendal.apache.org/docs/rust/opendal/services/index.html) for more information on how to setup the secrets for the storage service you want to connect to.
