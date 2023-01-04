use hyper::http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use rmps::Serializer;
use serde::{Deserialize, Serialize};

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

impl From<hyper::http::request::Parts> for RequestWrapper {
    fn from(parts: hyper::http::request::Parts) -> Self {
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
    pub fn into_request_builder(self) -> hyper::http::request::Builder {
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

impl From<hyper::http::response::Parts> for ResponseWrapper {
    fn from(parts: hyper::http::response::Parts) -> Self {
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
    pub fn into_response_builder(self) -> hyper::http::response::Builder {
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

#[derive(Clone, Debug, PartialEq)]
struct Log {
    message: String,
}

impl Log {
    pub fn into_bytes(self) -> Vec<u8> {
        let mut buf = Vec::new();

        self.append_bytes(&mut buf);

        buf
    }
}

trait Bytesable {
    /// Add self to bytes vec
    fn append_bytes(self, buf: &mut Vec<u8>);

    /// Get self from bytes vec
    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Self;
}

impl Bytesable for usize {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_le_bytes());
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Self {
        let mut buf = [0; usize::BITS as usize / 8];
        buf.fill_with(|| iter.next().unwrap());

        usize::from_le_bytes(buf)
    }
}

impl Bytesable for String {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        self.len().append_bytes(buf);
        buf.extend_from_slice(self.as_bytes());
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Self {
        let length = usize::from_bytes(iter);

        let mut vec = vec![0; length];
        vec.fill_with(|| iter.next().unwrap());

        String::from_utf8(vec).unwrap()
    }
}

impl Bytesable for Log {
    fn append_bytes(self, buf: &mut Vec<u8>) {
        self.message.append_bytes(buf);
    }

    fn from_bytes<I: Iterator<Item = u8>>(iter: &mut I) -> Self {
        Self {
            message: String::from_bytes(iter),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    use super::*;
    use hyper::body::Body;
    use hyper::http::HeaderValue;

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
            message: "Hello test".to_string(),
        };

        let mut buf = Vec::new();
        log.clone().append_bytes(&mut buf);
        let mut iter = buf.into_iter();

        let actual = Log::from_bytes(&mut iter);

        assert_eq!(log, actual);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn logs_over_socket() {
        let (rx, mut tx) = UnixStream::pair().unwrap();
        let log1 = Log {
            message: "First message".to_string(),
        };
        let log2 = Log {
            message: "Second message".to_string(),
        };

        tx.write(&log1.clone().into_bytes()).unwrap();
        tx.write(&log2.clone().into_bytes()).unwrap();

        let mut rx = rx.bytes().filter_map(Result::ok);

        let actual = Log::from_bytes(&mut rx);
        assert_eq!(log1, actual);

        let actual = Log::from_bytes(&mut rx);
        assert_eq!(log2, actual);
    }
}
