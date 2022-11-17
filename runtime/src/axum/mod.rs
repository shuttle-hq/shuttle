use std::convert::Infallible;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::os::unix::prelude::RawFd;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cap_std::os::unix::net::UnixStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use shuttle_common::wasm::{RequestWrapper, ResponseWrapper};
use shuttle_proto::runtime::runtime_server::Runtime;
use shuttle_proto::runtime::{
    self, LoadRequest, LoadResponse, StartRequest, StartResponse, SubscribeLogsRequest,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;
use tracing::info;
use wasi_common::file::FileCaps;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::sync::net::UnixStream as WasiUnixStream;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

extern crate rmp_serde as rmps;

pub struct AxumWasm {
    router: std::sync::Mutex<Option<Router>>,
    port: Mutex<Option<u16>>,
}

impl AxumWasm {
    pub fn new() -> Self {
        Self {
            router: std::sync::Mutex::new(None),
            port: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait]
impl Runtime for AxumWasm {
    async fn load(
        &self,
        request: tonic::Request<LoadRequest>,
    ) -> Result<tonic::Response<LoadResponse>, Status> {
        let wasm_path = request.into_inner().path;
        info!(wasm_path, "loading");

        let router = Router::new(wasm_path);

        *self.router.lock().unwrap() = Some(router);

        let message = LoadResponse { success: true };

        Ok(tonic::Response::new(message))
    }

    async fn start(
        &self,
        _request: tonic::Request<StartRequest>,
    ) -> Result<tonic::Response<StartResponse>, Status> {
        let port = 7002;
        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);

        let router = self.router.lock().unwrap().take().unwrap().inner;

        let make_service = make_service_fn(move |_conn| {
            let router = router.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                    let router = router.clone();
                    async move {
                        Ok::<_, Infallible>(router.lock().await.send_request(req).await.unwrap())
                    }
                }))
            }
        });

        info!("starting hyper server on: {}", &address);
        let server = hyper::Server::bind(&address).serve(make_service);

        _ = tokio::spawn(server);

        *self.port.lock().unwrap() = Some(port);

        let message = StartResponse {
            success: true,
            port: port as u32,
        };

        Ok(tonic::Response::new(message))
    }

    type SubscribeLogsStream = ReceiverStream<Result<runtime::LogItem, Status>>;

    async fn subscribe_logs(
        &self,
        _request: tonic::Request<SubscribeLogsRequest>,
    ) -> Result<tonic::Response<Self::SubscribeLogsStream>, Status> {
        todo!()
    }
}

struct RouterBuilder {
    engine: Engine,
    store: Store<WasiCtx>,
    linker: Linker<WasiCtx>,
    src: Option<File>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        let engine = Engine::default();

        let mut linker: Linker<WasiCtx> = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s).unwrap();

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_args()
            .unwrap()
            .build();

        let store = Store::new(&engine, wasi);

        Self {
            engine,
            store,
            linker,
            src: None,
        }
    }

    pub fn src<P: AsRef<Path>>(mut self, src: P) -> Self {
        self.src = Some(File::open(src).unwrap());
        self
    }

    pub fn build(mut self) -> Router {
        let mut buf = Vec::new();
        self.src.unwrap().read_to_end(&mut buf).unwrap();
        let module = Module::new(&self.engine, buf).unwrap();

        for export in module.exports() {
            println!("export: {}", export.name());
        }

        self.linker
            .module(&mut self.store, "axum", &module)
            .unwrap();
        let inner = RouterInner {
            store: self.store,
            linker: self.linker,
        };
        Router {
            inner: Arc::new(tokio::sync::Mutex::new(inner)),
        }
    }
}

struct RouterInner {
    store: Store<WasiCtx>,
    linker: Linker<WasiCtx>,
}

impl RouterInner {
    /// Send a HTTP request with body to given endpoint on the axum-wasm router and return the response
    pub async fn send_request(
        &mut self,
        req: hyper::Request<Body>,
    ) -> Result<Response<Body>, Infallible> {
        let (mut parts_stream, parts_client) = UnixStream::pair().unwrap();
        let (mut body_stream, body_client) = UnixStream::pair().unwrap();

        let parts_client = WasiUnixStream::from_cap_std(parts_client);
        let body_client = WasiUnixStream::from_cap_std(body_client);

        self.store
            .data_mut()
            .insert_file(3, Box::new(parts_client), FileCaps::all());

        self.store
            .data_mut()
            .insert_file(4, Box::new(body_client), FileCaps::all());

        let (parts, body) = req.into_parts();

        // serialise request parts to rmp
        let request_rmp = RequestWrapper::from(parts).into_rmp();

        // write request parts
        parts_stream.write_all(&request_rmp).unwrap();

        // write body
        body_stream
            .write_all(hyper::body::to_bytes(body).await.unwrap().as_ref())
            .unwrap();
        // signal to the receiver that end of file has been reached
        body_stream.write(&[0]).unwrap();

        println!("calling inner Router");
        self.linker
            .get(&mut self.store, "axum", "__SHUTTLE_Axum_call")
            .unwrap()
            .into_func()
            .unwrap()
            .typed::<(RawFd, RawFd), (), _>(&self.store)
            .unwrap()
            .call(&mut self.store, (3, 4))
            .unwrap();

        // read response parts from host
        let reader = BufReader::new(&mut parts_stream);

        // deserialize response parts from rust messagepack
        let wrapper: ResponseWrapper = rmps::from_read(reader).unwrap();

        // read response body from wasm router
        let mut body_buf = Vec::new();
        let mut c_buf: [u8; 1] = [0; 1];
        loop {
            body_stream.read(&mut c_buf).unwrap();
            if c_buf[0] == 0 {
                break;
            } else {
                body_buf.push(c_buf[0]);
            }
        }

        let response: Response<Body> = wrapper
            .into_response_builder()
            .body(body_buf.into())
            .unwrap();

        Ok(response)
    }
}

#[derive(Clone)]
struct Router {
    inner: Arc<tokio::sync::Mutex<RouterInner>>,
}

impl Router {
    pub fn builder() -> RouterBuilder {
        RouterBuilder::new()
    }

    pub fn new<P: AsRef<Path>>(src: P) -> Self {
        Self::builder().src(src).build()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use hyper::{http::HeaderValue, Method, Request, StatusCode, Version};

    #[tokio::test]
    async fn axum() {
        let axum = Router::new("axum.wasm");
        let mut inner = axum.inner.lock().await;

        // GET /hello
        let request: Request<Body> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .uri(format!("https://axum-wasm.example/hello"))
            .body(Body::empty())
            .unwrap();

        let res = inner.send_request(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            &hyper::body::to_bytes(res.into_body())
                .await
                .unwrap()
                .iter()
                .cloned()
                .collect::<Vec<u8>>()
                .as_ref(),
            b"Hello, World!"
        );

        // GET /goodbye
        let request: Request<Body> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("goodbye"))
            .uri(format!("https://axum-wasm.example/goodbye"))
            .body(Body::from("Goodbye world body"))
            .unwrap();

        let res = inner.send_request(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            &hyper::body::to_bytes(res.into_body())
                .await
                .unwrap()
                .iter()
                .cloned()
                .collect::<Vec<u8>>()
                .as_ref(),
            b"Goodbye, World!"
        );

        // GET /invalid
        let request: Request<Body> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("invalid"))
            .uri(format!("https://axum-wasm.example/invalid"))
            .body(Body::empty())
            .unwrap();

        let res = inner.send_request(request).await.unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
