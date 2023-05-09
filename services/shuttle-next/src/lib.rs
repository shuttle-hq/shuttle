//! [shuttle_next](https://docs.shuttle.rs/examples/shuttle-next)
//! A batteries-included, WASM-based backend web-framework.
pub use axum::*;
pub use futures_executor::block_on;
pub use http::Request;
pub use rmp_serde::from_read;
pub use shuttle_codegen::app;
pub use shuttle_common::wasm::{Logger, RequestWrapper, ResponseWrapper};
pub use tower_service::Service;
pub use tracing_subscriber::{prelude as tracing_prelude, registry as tracing_registry};
