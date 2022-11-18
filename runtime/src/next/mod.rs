use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::ops::DerefMut;
use std::os::unix::prelude::RawFd;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use cap_std::os::unix::net::UnixStream;
use serenity::{model::prelude::*, prelude::*};
use shuttle_proto::runtime::runtime_server::Runtime;
use shuttle_proto::runtime::{
    self, LoadRequest, LoadResponse, StartRequest, StartResponse, StopRequest, StopResponse,
    SubscribeLogsRequest,
};
use shuttle_service::ServiceName;
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::trace;
use wasi_common::file::FileCaps;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::sync::net::UnixStream as WasiUnixStream;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

pub struct Next {
    bot: std::sync::Mutex<Option<Bot>>,
    kill_tx: std::sync::Mutex<Option<oneshot::Sender<String>>>,
}

impl Next {
    pub fn new() -> Self {
        Self {
            bot: std::sync::Mutex::new(None),
            kill_tx: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait]
impl Runtime for Next {
    async fn load(&self, request: Request<LoadRequest>) -> Result<Response<LoadResponse>, Status> {
        let wasm_path = request.into_inner().path;
        trace!(wasm_path, "loading");

        let bot = Bot::new(wasm_path);

        *self.bot.lock().unwrap() = Some(bot);

        let message = LoadResponse { success: true };

        Ok(Response::new(message))
    }

    async fn start(
        &self,
        _request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
        let token = env::var("DISCORD_TOKEN").unwrap();
        let bot: Bot = {
            let guard = self.bot.lock().unwrap();
            guard.as_ref().unwrap().clone()
        };
        let client = bot.into_client(token.as_str(), intents).await;

        let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();

        *self.kill_tx.lock().unwrap() = Some(kill_tx);

        // start bot as a background task with a kill receiver
        trace!("starting bot");
        tokio::spawn(run_until_stopped(client, kill_rx));

        let message = StartResponse {
            success: true,
            // todo: port set here until I can set the port field to optional in the protobuf
            port: 8001,
        };

        Ok(Response::new(message))
    }

    type SubscribeLogsStream = ReceiverStream<Result<runtime::LogItem, Status>>;

    async fn subscribe_logs(
        &self,
        _request: Request<SubscribeLogsRequest>,
    ) -> Result<Response<Self::SubscribeLogsStream>, Status> {
        todo!()
    }

    async fn stop(&self, request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        let request = request.into_inner();

        let service_name = ServiceName::from_str(request.service_name.as_str())
            .map_err(|err| Status::from_error(Box::new(err)))?;

        let kill_tx = self.kill_tx.lock().unwrap().deref_mut().take();

        if let Some(kill_tx) = kill_tx {
            tokio::spawn(async move {
                kill_tx
                    .send(format!("stopping deployment: {}", &service_name))
                    .unwrap();
            });

            Ok(Response::new(StopResponse { success: true }))
        } else {
            Err(Status::internal("failed to stop deployment"))
        }
    }
}

/// Run the bot until a stop signal is received
async fn run_until_stopped(mut client: Client, kill_rx: tokio::sync::oneshot::Receiver<String>) {
    tokio::select! {
        _ = client.start() => {
            trace!("serenity bot stopped");
        },
        message = kill_rx => {
            match message {
                Ok(msg) => trace!("{msg}"),
                Err(_) => trace!("the sender dropped")
            }
        }
    }
}

struct BotBuilder {
    engine: Engine,
    store: Store<WasiCtx>,
    linker: Linker<WasiCtx>,
    src: Option<File>,
}

impl BotBuilder {
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

    pub fn build(mut self) -> Bot {
        let mut buf = Vec::new();
        self.src.unwrap().read_to_end(&mut buf).unwrap();
        let module = Module::new(&self.engine, buf).unwrap();

        for export in module.exports() {
            println!("export: {}", export.name());
        }

        self.linker.module(&mut self.store, "bot", &module).unwrap();
        let inner = BotInner {
            store: self.store,
            linker: self.linker,
        };
        Bot {
            inner: Arc::new(Mutex::new(inner)),
        }
    }
}

struct BotInner {
    store: Store<WasiCtx>,
    linker: Linker<WasiCtx>,
}

impl BotInner {
    pub async fn message(&mut self, new_message: &str) -> Option<String> {
        let (mut host, client) = UnixStream::pair().unwrap();
        let client = WasiUnixStream::from_cap_std(client);

        self.store
            .data_mut()
            .insert_file(3, Box::new(client), FileCaps::all());

        host.write_all(new_message.as_bytes()).unwrap();
        host.write(&[0]).unwrap();

        println!("calling inner EventHandler message");
        self.linker
            .get(&mut self.store, "bot", "__SHUTTLE_EventHandler_message")
            .unwrap()
            .into_func()
            .unwrap()
            .typed::<RawFd, (), _>(&self.store)
            .unwrap()
            .call(&mut self.store, 3)
            .unwrap();

        let mut resp = String::new();
        host.read_to_string(&mut resp).unwrap();

        if resp.is_empty() {
            None
        } else {
            Some(resp)
        }
    }
}

#[derive(Clone)]
struct Bot {
    inner: Arc<Mutex<BotInner>>,
}

impl Bot {
    pub fn builder() -> BotBuilder {
        BotBuilder::new()
    }

    pub fn new<P: AsRef<Path>>(src: P) -> Self {
        Self::builder().src(src).build()
    }

    pub async fn into_client(self, token: &str, intents: GatewayIntents) -> Client {
        Client::builder(&token, intents)
            .event_handler(self)
            .await
            .unwrap()
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, new_message: Message) {
        let mut inner = self.inner.lock().await;
        if let Some(resp) = inner.message(new_message.content.as_str()).await {
            new_message.channel_id.say(&ctx.http, resp).await.unwrap();
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn bot() {
        let bot = Bot::new("bot.wasm");
        let mut inner = bot.inner.lock().await;
        assert_eq!(inner.message("not !hello").await, None);
        assert_eq!(inner.message("!hello").await, Some("world!".to_string()));
    }
}
