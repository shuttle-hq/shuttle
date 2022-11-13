use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::prelude::RawFd;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cap_std::os::unix::net::UnixStream;
use http_body::Full;
use hyper::body::Bytes;
use hyper::Response;
use shuttle_axum_utils::{wrap_request, RequestWrapper, ResponseWrapper};
use shuttle_proto::runtime::runtime_server::Runtime;
use shuttle_proto::runtime::{LoadRequest, LoadResponse, StartRequest, StartResponse};
use tonic::Status;
use tracing::trace;
use wasi_common::file::FileCaps;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::sync::net::UnixStream as WasiUnixStream;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

pub struct AxumWasm {
    router: std::sync::Mutex<Option<Router>>,
}

impl AxumWasm {
    pub fn new() -> Self {
        Self {
            router: std::sync::Mutex::new(None),
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
        trace!(wasm_path, "loading");

        let router = Router::new(wasm_path);

        *self.router.lock().unwrap() = Some(router);

        let message = LoadResponse { success: true };

        Ok(tonic::Response::new(message))
    }

    async fn start(
        &self,
        _request: tonic::Request<StartRequest>,
    ) -> Result<tonic::Response<StartResponse>, Status> {
        // TODO: start a hyper server and serve the axum-wasm router as a service

        let message = StartResponse {
            success: true,
            port: 7002,
        };

        Ok(tonic::Response::new(message))
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
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

struct RouterInner {
    store: Store<WasiCtx>,
    linker: Linker<WasiCtx>,
}

impl RouterInner {
    /// Send a HTTP request with body to given endpoint on the axum-wasm router and return the response
    pub async fn send_request(&mut self, req: hyper::Request<Full<Bytes>>) -> Response<Vec<u8>> {
        let (mut host, client) = UnixStream::pair().unwrap();
        let client = WasiUnixStream::from_cap_std(client);

        self.store
            .data_mut()
            .insert_file(3, Box::new(client), FileCaps::all());

        // serialise request to rmp
        let request_rmp = wrap_request(req).await.into_rmp();

        host.write_all(&request_rmp).unwrap();
        host.write(&[0]).unwrap();

        println!("calling inner Router");
        self.linker
            .get(&mut self.store, "axum", "__SHUTTLE_Axum_call")
            .unwrap()
            .into_func()
            .unwrap()
            .typed::<RawFd, (), _>(&self.store)
            .unwrap()
            .call(&mut self.store, 3)
            .unwrap();

        let mut res_buf = Vec::new();
        host.read_to_end(&mut res_buf).unwrap();

        // deserialize response from rmp
        let res = ResponseWrapper::from_rmp(res_buf);

        // consume the wrapper and return response
        res.into_response()
    }
}

#[derive(Clone)]
struct Router {
    inner: Arc<Mutex<RouterInner>>,
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
    use hyper::{http::HeaderValue, Method, Request, StatusCode, Version};

    use super::*;

    #[tokio::test]
    async fn axum() {
        let axum = Router::new("axum.wasm");
        let mut inner = axum.inner.lock().unwrap();

        // GET /hello
        let request: Request<Full<Bytes>> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("hello"))
            .uri(format!("https://axum-wasm.example/hello"))
            .body(Full::new(Bytes::from_static(b"some body")))
            .unwrap();

        let res = inner.send_request(request).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(std::str::from_utf8(&res.body()).unwrap(), "Hello, World!");

        // GET /goodbye
        let request: Request<Full<Bytes>> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("goodbye"))
            .uri(format!("https://axum-wasm.example/goodbye"))
            .body(Full::new(Bytes::from_static(b"some body")))
            .unwrap();

        let res = inner.send_request(request).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(std::str::from_utf8(&res.body()).unwrap(), "Goodbye, World!");

        // GET /invalid
        let request: Request<Full<Bytes>> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("invalid"))
            .uri(format!("https://axum-wasm.example/invalid"))
            .body(Full::new(Bytes::from_static(b"some body")))
            .unwrap();

        let res = inner.send_request(request).await;

        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
