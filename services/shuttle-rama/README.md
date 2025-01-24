## Shuttle service integration for the Rama framework

Rama is still in early development and for now the latest
alpha release is used, `0.2.0-alpha.5`.

### Examples

#### Application Service

```rust,ignore
use rama::{http, Service, service::service_fn};

async fn hello_world() -> &'static str {
    "Hello, world!"
}

#[shuttle_runtime::main]
async fn main() -> Result<impl shuttle_rama::ShuttleService, shuttle_rama::ShuttleError> {
    Ok(shuttle_rama::RamaService::application(
        service_fn(hello_world),
    ))
}
```

#### Transport Service

```rust,ignore
use rama::{net, Service, service::service_fn};
use std::convert::Infallible;
use tokio::io::AsyncWriteExt;

async fn hello_world(mut stream: impl net::stream::Socket + net::Stream + Unpin) -> Result<(), Infallible> {
    println!(
        "Incoming connection from: {}",
        stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "???".to_owned())
    );

    const TEXT: &str = "Hello, Shuttle!";

    let resp = [
        "HTTP/1.1 200 OK",
        "Content-Type: text/plain",
        format!("Content-Length: {}", TEXT.len()).as_str(),
        "",
        TEXT,
        "",
    ]
    .join("\r\n");

    stream
        .write_all(resp.as_bytes())
        .await
        .expect("write to stream");

    Ok::<_, std::convert::Infallible>(())
}

#[shuttle_runtime::main]
async fn main() -> Result<impl shuttle_rama::ShuttleService, shuttle_rama::ShuttleError> {
    Ok(shuttle_rama::RamaService::transport(
        service_fn(hello_world),
    ))
}
```
