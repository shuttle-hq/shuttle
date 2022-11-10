use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::prelude::RawFd;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cap_std::os::unix::net::UnixStream;
use shuttle_proto::runtime::runtime_server::Runtime;
use shuttle_proto::runtime::{LoadRequest, LoadResponse, StartRequest, StartResponse};
use tonic::{Request, Response, Status};
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
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let wasm_path = request.into_inner().path;
        trace!(wasm_path, "loading");

        let router = Router::new(wasm_path);

        *self.router.lock().unwrap() = Some(router);

        let message = LoadResponse { success: true };

        Ok(Response::new(message))
    }

    async fn start(
        &self,
        _request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        // TODO: start a process that streams requests from a socket into wasm router and returns response?

        let message = StartResponse {
            success: true,
            port: 7002,
        };

        Ok(Response::new(message))
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
    /// Send a GET request to given endpoint on the axum-wasm router
    pub async fn get(&mut self, endpoint: &str) -> Option<String> {
        let (mut host, client) = UnixStream::pair().unwrap();
        let client = WasiUnixStream::from_cap_std(client);

        self.store
            .data_mut()
            .insert_file(3, Box::new(client), FileCaps::all());

        host.write_all(endpoint.as_bytes()).unwrap();
        host.write(&[0]).unwrap();

        println!("calling inner Router endpoint: /{endpoint}");

        self.linker
            .get(&mut self.store, "axum", "__SHUTTLE_Axum_call")
            .unwrap()
            .into_func()
            .unwrap()
            .typed::<RawFd, (), _>(&self.store)
            .unwrap()
            .call(&mut self.store, 3)
            .unwrap();

        let mut res = String::new();
        host.read_to_string(&mut res).unwrap();

        if res.is_empty() {
            println!("invalid endpoint");
            None
        } else {
            println!("received response: {res}");
            Some(res)
        }
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
    use super::*;

    #[tokio::test]
    async fn axum() {
        let axum = Router::new("axum.wasm");
        let mut inner = axum.inner.lock().unwrap();

        assert_eq!(inner.get("hello").await, Some("Hello, World!".to_string()));
        assert_eq!(
            inner.get("goodbye").await,
            Some("Goodbye, World!".to_string())
        );
        assert_eq!(inner.get("not/a/real/endpoint").await, None);
    }
}
