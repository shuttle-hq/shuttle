# Shuttle Service Info

This plugin allows applications to obtain certain information about their runtime environment.

## Usage

Add `shuttle-metadata` to the dependencies for your service.

You can get this resource using the `shuttle_metadata::ShuttleServiceInfo` attribute to get a `ServiceInfo`. This struct will contain information such as the Shuttle service name.

```rust
#[shuttle_runtime::main]
async fn app(
    #[shuttle_metadata::ShuttleServiceInfo] service_info: shuttle_metadata::ServiceInfo,
) -> __ { ... }
```

#### Example projects that use `shuttle-metadata`

| Framework | Link                                                                                   |
| --------- | -------------------------------------------------------------------------------------- |
| Axum      | [axum example](https://github.com/shuttle-hq/shuttle-examples/tree/main/axum/metadata) |
