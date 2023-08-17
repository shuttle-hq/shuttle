# Shuttle Service Info

This plugin allows applications to obtain certain information about their runtime environment.

## Usage

Add `shuttle-service-info` to the dependencies for your service.

You can get this resource using the `shuttle_service_info::ShuttleServiceInfo` attribute to get a `ServiceInfo`. This struct will contain information such as the Shuttle service name.

```rust
#[shuttle_runtime::main]
async fn app(
    #[shuttle_service_info::ShuttleServiceInfo] service_info: shuttle_service_info::ServiceInfo,
) -> __ { ... }
```

#### Example projects that use `shuttle-service-info`

| Framework | Link                                                                                       |
| --------- | ------------------------------------------------------------------------------------------ |
| Axum      | [axum example](https://github.com/shuttle-hq/shuttle-examples/tree/main/axum/service-info) |
