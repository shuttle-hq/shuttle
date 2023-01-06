use std::convert::Infallible;
use std::io::{BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr};
use std::ops::DerefMut;
use std::os::unix::prelude::RawFd;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;

use async_trait::async_trait;
use cap_std::os::unix::net::UnixStream;
use futures::TryStreamExt;
use hyper::body::HttpBody;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use shuttle_common::wasm::{Bytesable, Log, RequestWrapper, ResponseWrapper};
use shuttle_proto::runtime::runtime_server::Runtime;
use shuttle_proto::runtime::{
    self, LoadRequest, LoadResponse, StartRequest, StartResponse, StopRequest, StopResponse,
    SubscribeLogsRequest,
};
use shuttle_service::ServiceName;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tonic::Status;
use tracing::{error, trace};
use wasi_common::file::FileCaps;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::sync::net::UnixStream as WasiUnixStream;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

extern crate rmp_serde as rmps;

pub struct AxumWasm {
    router: Mutex<Option<Router>>,
    logs_rx: Mutex<Option<Receiver<Result<runtime::LogItem, Status>>>>,
    logs_tx: Mutex<Sender<Result<runtime::LogItem, Status>>>,
    kill_tx: Mutex<Option<oneshot::Sender<String>>>,
}

impl AxumWasm {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(1);

        Self {
            router: Mutex::new(None),
            logs_rx: Mutex::new(Some(rx)),
            logs_tx: Mutex::new(tx),
            kill_tx: Mutex::new(None),
        }
    }
}

impl Default for AxumWasm {
    fn default() -> Self {
        Self::new()
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
        request: tonic::Request<StartRequest>,
    ) -> Result<tonic::Response<StartResponse>, Status> {
        let StartRequest {
            deployment_id,
            port,
            ..
        } = request.into_inner();

        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port as u16);

        let router = self.router.lock().unwrap().take().unwrap();
        let logs_tx = self.logs_tx.lock().unwrap().clone();

        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();

        *self.kill_tx.lock().unwrap() = Some(kill_tx);

        // TODO: split `into_server` up into build and run functions
        tokio::spawn(router.into_server(deployment_id, address, logs_tx, kill_rx));

        let message = StartResponse { success: true };

        Ok(tonic::Response::new(message))
    }

    type SubscribeLogsStream = ReceiverStream<Result<runtime::LogItem, Status>>;

    async fn subscribe_logs(
        &self,
        _request: tonic::Request<SubscribeLogsRequest>,
    ) -> Result<tonic::Response<Self::SubscribeLogsStream>, Status> {
        let logs_rx = self.logs_rx.lock().unwrap().deref_mut().take();

        if let Some(logs_rx) = logs_rx {
            Ok(tonic::Response::new(ReceiverStream::new(logs_rx)))
        } else {
            Err(Status::internal("logs have already been subscribed to"))
        }
    }

    async fn stop(
        &self,
        request: tonic::Request<StopRequest>,
    ) -> Result<tonic::Response<StopResponse>, Status> {
        let request = request.into_inner();

        let service_name = ServiceName::from_str(request.service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let kill_tx = self.kill_tx.lock().unwrap().deref_mut().take();

        if let Some(kill_tx) = kill_tx {
            if kill_tx
                .send(format!("stopping deployment: {}", &service_name))
                .is_err()
            {
                error!("the receiver dropped");
                return Err(Status::internal("failed to stop deployment"));
            }

            Ok(tonic::Response::new(StopResponse { success: true }))
        } else {
            Err(Status::internal("failed to stop deployment"))
        }
    }
}

struct RouterBuilder {
    engine: Engine,
    linker: Linker<WasiCtx>,
    src: Option<PathBuf>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        let engine = Engine::default();

        let mut linker: Linker<WasiCtx> = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s).unwrap();

        Self {
            engine,
            linker,
            src: None,
        }
    }

    pub fn src<P: AsRef<Path>>(mut self, src: P) -> Self {
        self.src = Some(src.as_ref().to_path_buf());
        self
    }

    pub fn build(self) -> Router {
        let file = self.src.unwrap();
        let module = Module::from_file(&self.engine, file).unwrap();

        for export in module.exports() {
            println!("export: {}", export.name());
        }
        let inner = RouterInner {
            linker: self.linker,
            engine: self.engine,
            module,
        };
        Router { inner }
    }
}

#[derive(Clone)]
struct RouterInner {
    linker: Linker<WasiCtx>,
    engine: Engine,
    module: Module,
}

impl RouterInner {
    /// Send a HTTP request with body to given endpoint on the axum-wasm router and return the response
    pub async fn handle_request(
        &mut self,
        deployment_id: Vec<u8>,
        req: hyper::Request<Body>,
        logs_tx: Sender<Result<runtime::LogItem, Status>>,
    ) -> Result<Response<Body>, Infallible> {
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_args()
            .unwrap()
            .build();

        let mut store = Store::new(&self.engine, wasi);
        self.linker
            .module(&mut store, "axum", &self.module)
            .unwrap();

        let (logs_stream, logs_client) = UnixStream::pair().unwrap();
        let (mut parts_stream, parts_client) = UnixStream::pair().unwrap();
        let (mut body_write_stream, body_write_client) = UnixStream::pair().unwrap();
        let (body_read_stream, body_read_client) = UnixStream::pair().unwrap();

        let logs_client = WasiUnixStream::from_cap_std(logs_client);
        let parts_client = WasiUnixStream::from_cap_std(parts_client);
        let body_write_client = WasiUnixStream::from_cap_std(body_write_client);
        let body_read_client = WasiUnixStream::from_cap_std(body_read_client);

        store
            .data_mut()
            .insert_file(2, Box::new(logs_client), FileCaps::all());

        store
            .data_mut()
            .insert_file(3, Box::new(parts_client), FileCaps::all());
        store
            .data_mut()
            .insert_file(4, Box::new(body_write_client), FileCaps::all());
        store
            .data_mut()
            .insert_file(5, Box::new(body_read_client), FileCaps::all());

        tokio::task::spawn(async move {
            let mut iter = logs_stream.bytes().filter_map(Result::ok);

            while let Some(log) = Log::from_bytes(&mut iter) {
                let mut log: runtime::LogItem = log.into();
                log.id = deployment_id.clone();

                logs_tx.send(Ok(log)).await.unwrap();
            }
        });

        let (parts, body) = req.into_parts();

        // serialise request parts to rmp
        let request_rmp = RequestWrapper::from(parts).into_rmp();

        // write request parts
        parts_stream.write_all(&request_rmp).unwrap();

        // To protect our server, reject requests with bodies larger than
        // 64kbs of data.
        let body_size = body.size_hint().upper().unwrap_or(u64::MAX);

        if body_size > 1024 * 64 {
            let response = Response::builder()
                .status(hyper::http::StatusCode::PAYLOAD_TOO_LARGE)
                .body(Body::empty())
                .unwrap();

            // Return early if body is too big
            return Ok(response);
        }

        let body_bytes = hyper::body::to_bytes(body).await.unwrap();

        // write body to axum
        body_write_stream.write_all(body_bytes.as_ref()).unwrap();

        // drop stream to signal EOF
        drop(body_write_stream);

        println!("calling inner Router");
        self.linker
            .get(&mut store, "axum", "__SHUTTLE_Axum_call")
            .unwrap()
            .into_func()
            .unwrap()
            .typed::<(RawFd, RawFd, RawFd, RawFd), ()>(&store)
            .unwrap()
            .call(&mut store, (2, 3, 4, 5))
            .unwrap();

        // read response parts from host
        let reader = BufReader::new(&mut parts_stream);

        // deserialize response parts from rust messagepack
        let wrapper: ResponseWrapper = rmps::from_read(reader).unwrap();

        // read response body from wasm and stream it to our hyper server
        let reader = BufReader::new(body_read_stream);
        let stream = futures::stream::iter(reader.bytes()).try_chunks(2);
        let body = hyper::Body::wrap_stream(stream);

        let response: Response<Body> = wrapper.into_response_builder().body(body).unwrap();

        Ok(response)
    }
}

#[derive(Clone)]
struct Router {
    inner: RouterInner,
}

impl Router {
    pub fn builder() -> RouterBuilder {
        RouterBuilder::new()
    }

    pub fn new<P: AsRef<Path>>(src: P) -> Self {
        Self::builder().src(src).build()
    }

    /// Consume the router, build and run server until a stop signal is received via the
    /// kill receiver
    // TODO: figure out how to handle the complicated generics for hyper::Server and
    // hyper::MakeServiceFn and split this up into `build` and `run_until_stopped` functions
    pub async fn into_server(
        self,
        deployment_id: Vec<u8>,
        address: SocketAddr,
        logs_tx: Sender<Result<runtime::LogItem, Status>>,
        kill_rx: tokio::sync::oneshot::Receiver<String>,
    ) {
        let router = self.inner;

        let make_service = make_service_fn(move |_conn| {
            let deployment_id = deployment_id.clone();
            let router = router.clone();
            let logs_tx = logs_tx.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                    let deployment_id = deployment_id.clone();
                    let mut router = router.clone();
                    let logs_tx = logs_tx.clone();
                    async move {
                        Ok::<_, Infallible>(
                            router
                                .handle_request(deployment_id, req, logs_tx)
                                .await
                                .unwrap(),
                        )
                    }
                }))
            }
        });

        let server = hyper::Server::bind(&address).serve(make_service);

        trace!("starting hyper server on: {}", &address);
        tokio::select! {
            _ = server => {
                trace!("axum wasm server stopped");
            },
            message = kill_rx => {
                match message {
                    Ok(msg) => trace!("{msg}"),
                    Err(_) => trace!("the sender dropped")
                }
            }
        };
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use hyper::{http::HeaderValue, Method, Request, StatusCode, Version};
    use uuid::Uuid;

    #[tokio::test]
    async fn axum() {
        let axum = Router::new("axum.wasm");
        let inner = axum.inner;
        let id = Uuid::default().as_bytes().to_vec();
        let (tx, mut rx) = mpsc::channel(1);

        tokio::spawn(async move {
            while let Some(log) = rx.recv().await {
                println!("{log:?}");
            }
        });

        // GET /hello
        let request: Request<Body> = Request::builder()
            .method(Method::GET)
            .version(Version::HTTP_11)
            .uri("https://axum-wasm.example/hello")
            .body(Body::empty())
            .unwrap();

        let res = inner
            .clone()
            .handle_request(id.clone(), request, tx.clone())
            .await
            .unwrap();

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
            .uri("https://axum-wasm.example/goodbye")
            .body(Body::from("Goodbye world body"))
            .unwrap();

        let res = inner
            .clone()
            .handle_request(id.clone(), request, tx.clone())
            .await
            .unwrap();

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
            .uri("https://axum-wasm.example/invalid")
            .body(Body::empty())
            .unwrap();

        let res = inner
            .clone()
            .handle_request(id.clone(), request, tx.clone())
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::NOT_FOUND);

        // POST /uppercase
        let request: Request<Body> = Request::builder()
            .method(Method::POST)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("invalid"))
            .uri("https://axum-wasm.example/uppercase")
            .body("this should be uppercased".into())
            .unwrap();

        let res = inner.clone().handle_request(id, request, tx).await.unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            &hyper::body::to_bytes(res.into_body())
                .await
                .unwrap()
                .iter()
                .cloned()
                .collect::<Vec<u8>>()
                .as_ref(),
            b"THIS SHOULD BE UPPERCASED"
        );
    }
}
