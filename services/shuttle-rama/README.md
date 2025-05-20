## Shuttle service integration for the Rama framework

Learn more about rama at <https://ramaproxy.org/> and see more [Rama v0.2] examples
at <https://github.com/plabayo/rama/tree/rama-0.2.0/examples>.

[Rama]: https://github.com/plabayo/rama

### Examples

#### Application Service

```rust,ignore
use rama::{
    Context, Layer,
    error::ErrorContext,
    http::{
        StatusCode,
        layer::forwarded::GetForwardedHeaderLayer,
        service::web::{Router, response::Result},
    },
    net::forwarded::Forwarded,
};

async fn hello_world(ctx: Context<()>) -> Result<String> {
    Ok(match ctx.get::<Forwarded>() {
        Some(forwarded) => format!(
            "Hello cloud user @ {}!",
            forwarded
                .client_ip()
                .context("missing IP information from user")
                .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
        ),
        None => "Hello local user! Are you developing?".to_owned(),
    })
}

#[shuttle_runtime::main]
async fn main() -> Result<impl shuttle_rama::ShuttleService, shuttle_rama::ShuttleError> {
    let router = Router::new().get("/", hello_world);

    let app =
        // Shuttle sits behind a load-balancer,
        // so in case you want the real IP of the user,
        // you need to ensure this headers is handled.
        //
        // Learn more at <https://docs.shuttle.dev/docs/deployment-environment#https-traffic>
        GetForwardedHeaderLayer::x_forwarded_for().into_layer(router);

    Ok(shuttle_rama::RamaService::application(app))
}
```

#### Transport Service

```rust,ignore
use rama::{net, service::service_fn};
use std::convert::Infallible;
use tokio::io::AsyncWriteExt;

async fn hello_world<S>(mut stream: S) -> Result<(), Infallible>
where
    S: net::stream::Stream + Unpin,
{
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
    Ok(shuttle_rama::RamaService::transport(service_fn(
        hello_world,
    )))
}
```
