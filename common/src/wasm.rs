use std::{
    io::Write,
    slice::IterMut,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, NaiveDateTime, Utc};
use http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use rmps::Serializer;
use serde::{Deserialize, Serialize};
use tracing::Subscriber;
use tracing_subscriber::Layer;

use crate::tracing::JsonVisitor;

extern crate rmp_serde as rmps;

// todo: add http extensions field
#[derive(Serialize, Deserialize, Debug)]
pub struct RequestWrapper {
    #[serde(with = "http_serde::method")]
    pub method: Method,

    #[serde(with = "http_serde::uri")]
    pub uri: Uri,

    #[serde(with = "http_serde::version")]
    pub version: Version,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
}

impl From<http::request::Parts> for RequestWrapper {
    fn from(parts: http::request::Parts) -> Self {
        RequestWrapper {
            method: parts.method,
            uri: parts.uri,
            version: parts.version,
            headers: parts.headers,
        }
    }
}

impl RequestWrapper {
    /// Serialize a RequestWrapper to the Rust MessagePack data format
    pub fn into_rmp(self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf)).unwrap();

        buf
    }

    /// Consume the wrapper and return a request builder with `Parts` set
    pub fn into_request_builder(self) -> http::request::Builder {
        let mut request = Request::builder()
            .method(self.method)
            .version(self.version)
            .uri(self.uri);

        request
            .headers_mut()
            .unwrap()
            .extend(self.headers.into_iter());

        request
    }
}

// todo: add http extensions field
#[derive(Serialize, Deserialize, Debug)]
pub struct ResponseWrapper {
    #[serde(with = "http_serde::status_code")]
    pub status: StatusCode,

    #[serde(with = "http_serde::version")]
    pub version: Version,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
}

impl From<http::response::Parts> for ResponseWrapper {
    fn from(parts: http::response::Parts) -> Self {
        ResponseWrapper {
            status: parts.status,
            version: parts.version,
            headers: parts.headers,
        }
    }
}

impl ResponseWrapper {
    /// Serialize a ResponseWrapper into the Rust MessagePack data format
    pub fn into_rmp(self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf)).unwrap();

        buf
    }

    /// Consume the wrapper and return a response builder with `Parts` set
    pub fn into_response_builder(self) -> http::response::Builder {
        let mut response = Response::builder()
            .status(self.status)
            .version(self.version);

        response
            .headers_mut()
            .unwrap()
            .extend(self.headers.into_iter());

        response
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Log {
    pub level: Level,
    pub timestamp: DateTime<Utc>,
    pub file: String,
    pub line: u32,
    pub target: String,
    pub fields: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<&tracing::Level> for Level {
    fn from(level: &tracing::Level) -> Self {
        match *level {
            tracing::Level::TRACE => Self::Trace,
            tracing::Level::DEBUG => Self::Debug,
            tracing::Level::INFO => Self::Info,
            tracing::Level::WARN => Self::Warn,
            tracing::Level::ERROR => Self::Error,
        }
    }
}

impl Log {
    pub fn into_bytes(self) -> Vec<u8> {
        let mut buf = Vec::new();

        buf.add(self);

        buf
    }
}

/// Like [slice::fill_with] but allows unwrapping of [Option]s
trait TryFillWith: Sized {
    fn try_fill_with<I: Iterator<Item = u8>>(self, iter: &mut I) -> Option<()>;
}

impl<'a> TryFillWith for IterMut<'a, u8> {
    fn try_fill_with<I: Iterator<Item = u8>>(self, iter: &mut I) -> Option<()> {
        for el in self {
            *el = iter.next()?;
        }

        Some(())
    }
}

/// Convert a structure to and from bytes (array of u8)
pub trait Bytesable: Sized {
    /// Add self to bytes vec
    fn append_bytes(self, buf: &mut Vec<u8>);

    /// Get self from bytes vec
    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self>;
}

macro_rules! impl_bytesable {
    ($($int:ident),*) => {
        $(impl Bytesable for $int {
            fn append_bytes(self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }

            fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self> {
                let mut buf = [0; $int::BITS as usize / 8];
                buf.iter_mut().try_fill_with(iter)?;

                Some($int::from_le_bytes(buf))
            }
        })*
    };
}

// Never implement for usize because it could map to u64 in the runtime and a u32 inside wasm
// This will cause the deserialization step to fail
impl_bytesable!(u32, u64, i64);

impl Bytesable for String {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        buf.add(self.len() as u64);
        buf.extend_from_slice(self.as_bytes());
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self> {
        let length: u64 = iter.get()?;

        let mut vec = vec![0; length as usize];
        vec.iter_mut().try_fill_with(iter)?;

        String::from_utf8(vec).ok()
    }
}

impl Bytesable for Level {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        buf.add(self as u32);
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self> {
        let i: u32 = iter.get()?;

        let res = match i {
            0 => Self::Trace,
            1 => Self::Debug,
            2 => Self::Info,
            3 => Self::Warn,
            4 => Self::Error,
            _ => Self::Trace,
        };

        Some(res)
    }
}

impl Bytesable for DateTime<Utc> {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        buf.add(self.naive_utc().timestamp_millis());
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self> {
        let millis: i64 = iter.get()?;

        let datetime = NaiveDateTime::from_timestamp_millis(millis)?;

        Some(Self::from_utc(datetime, Utc))
    }
}

impl Bytesable for Vec<u8> {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        buf.add(self.len() as u64);
        buf.extend_from_slice(&self);
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self> {
        let length: u64 = iter.get()?;

        let mut vec = vec![0; length as usize];
        vec.iter_mut().try_fill_with(iter)?;

        Some(vec)
    }
}

impl Bytesable for Log {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        buf.add(self.level);
        buf.add(self.timestamp);
        buf.add(self.file);
        buf.add(self.line);
        buf.add(self.target);
        buf.add(self.fields);
    }

    // These should be in the same order as they appear in [Self::append_bytes]
    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Option<Self> {
        Some(Self {
            level: iter.get()?,
            timestamp: iter.get()?,
            file: iter.get()?,
            line: iter.get()?,
            target: iter.get()?,
            fields: iter.get()?,
        })
    }
}

/// Trait to make it easier to add a bytable type to a data source
trait BytesableAppendExt {
    fn add<B: Bytesable>(&mut self, i: B);
}

impl BytesableAppendExt for Vec<u8> {
    fn add<B: Bytesable>(&mut self, i: B) {
        i.append_bytes(self);
    }
}

/// Trait to make it easier to get a bytable type from a data source
trait BytesableFromExt {
    fn get<B: Bytesable>(&mut self) -> Option<B>;
}

impl<I: Iterator<Item = u8>> BytesableFromExt for I {
    fn get<B: Bytesable>(&mut self) -> Option<B> {
        B::from_bytes(self)
    }
}

pub struct Logger<W> {
    writer: Arc<Mutex<W>>,
}

impl<W: Write> Logger<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

impl<S, W> Layer<S> for Logger<W>
where
    S: Subscriber,
    W: Write + 'static,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let datetime = Utc::now();

        let item = {
            let metadata = event.metadata();
            let mut visitor = JsonVisitor::default();

            event.record(&mut visitor);

            Log {
                level: metadata.level().into(),
                timestamp: datetime,
                file: visitor
                    .file
                    .or_else(|| metadata.file().map(str::to_string))
                    .unwrap_or_default(),
                line: visitor.line.or_else(|| metadata.line()).unwrap_or_default(),
                target: visitor
                    .target
                    .unwrap_or_else(|| metadata.target().to_string()),
                fields: serde_json::to_vec(&visitor.fields).unwrap(),
            }
        };

        let _ = self
            .writer
            .lock()
            .expect("to get lock on writer")
            .write(&item.into_bytes())
            .expect("sending log should succeed");
    }
}

#[cfg(test)]
mod test {
    use cap_std::os::unix::net::UnixStream;
    use serde_json::json;
    use std::io::{Read, Write};

    use super::*;
    use chrono::SubsecRound;
    use http::HeaderValue;
    use hyper::body::Body;
    use tracing_subscriber::prelude::*;

    #[test]
    fn request_roundtrip() {
        let request: Request<Body> = Request::builder()
            .method(Method::PUT)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("request"))
            .uri("https://axum-wasm.example/hello")
            .body(Body::empty())
            .unwrap();

        let (parts, _) = request.into_parts();
        let rmp = RequestWrapper::from(parts).into_rmp();

        let back: RequestWrapper = rmps::from_slice(&rmp).unwrap();

        assert_eq!(
            back.headers.get("test").unwrap(),
            HeaderValue::from_static("request")
        );
        assert_eq!(back.method, Method::PUT);
        assert_eq!(back.version, Version::HTTP_11);
        assert_eq!(
            back.uri.to_string(),
            "https://axum-wasm.example/hello".to_string()
        );
    }

    #[test]
    fn response_roundtrip() {
        let response: Response<Body> = Response::builder()
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("response"))
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap();

        let (parts, _) = response.into_parts();
        let rmp = ResponseWrapper::from(parts).into_rmp();

        let back: ResponseWrapper = rmps::from_slice(&rmp).unwrap();

        assert_eq!(
            back.headers.get("test").unwrap(),
            HeaderValue::from_static("response")
        );
        assert_eq!(back.status, StatusCode::NOT_MODIFIED);
        assert_eq!(back.version, Version::HTTP_11);
    }

    #[test]
    fn log_roundtrip() {
        let log = Log {
            level: Level::Debug,
            timestamp: Utc::now().trunc_subsecs(3),
            file: "main.rs".to_string(),
            line: 5,
            target: "crate::main".to_string(),
            fields: serde_json::to_vec(&json!({"message": "Hello"})).unwrap(),
        };

        let mut buf = Vec::new();
        buf.add(log.clone());
        let mut iter = buf.into_iter();

        let actual = iter.get::<Log>();

        assert_eq!(log, actual.unwrap());
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn logs_over_socket() {
        let (mut tx, rx) = UnixStream::pair().unwrap();
        let log1 = Log {
            level: Level::Debug,
            timestamp: Utc::now().trunc_subsecs(3),
            file: "lib.rs".to_string(),
            line: 9,
            target: "crate::lib".to_string(),
            fields: serde_json::to_vec(&json!({"message": "starting"})).unwrap(),
        };
        let log2 = Log {
            level: Level::Debug,
            timestamp: Utc::now().trunc_subsecs(3),
            file: Default::default(),
            line: Default::default(),
            target: Default::default(),
            fields: Default::default(),
        };

        let _ = tx.write(&log1.clone().into_bytes()).unwrap();
        let _ = tx.write(&log2.clone().into_bytes()).unwrap();

        let mut rx = rx.bytes().filter_map(Result::ok);

        let actual = rx.get::<Log>().unwrap();
        assert_eq!(log1, actual);

        let actual = rx.get::<Log>().unwrap();
        assert_eq!(log2, actual);

        // Make sure the closed channel (end) is handled correctly
        drop(tx);
        assert_eq!(rx.get::<Log>(), None);
    }

    #[test]
    fn logging() {
        let (tx, rx) = UnixStream::pair().unwrap();
        let mut rx = rx.bytes().filter_map(Result::ok);

        let logger = Logger::new(tx);
        let to_tuple = |log: Option<Log>| {
            let log = log.unwrap();
            let fields: serde_json::Map<String, serde_json::Value> =
                serde_json::from_slice(&log.fields).unwrap();

            let message = fields["message"].as_str().unwrap().to_owned();

            (message, log.level)
        };

        tracing_subscriber::registry().with(logger).init();

        tracing::debug!("this is");
        tracing::info!("hi");
        tracing::warn!("from");
        tracing::error!("logger");

        assert_eq!(
            to_tuple(rx.get::<Log>()),
            ("this is".to_string(), Level::Debug)
        );
        assert_eq!(to_tuple(rx.get::<Log>()), ("hi".to_string(), Level::Info));
        assert_eq!(to_tuple(rx.get::<Log>()), ("from".to_string(), Level::Warn));
        assert_eq!(
            to_tuple(rx.get::<Log>()),
            ("logger".to_string(), Level::Error)
        );
    }
}
