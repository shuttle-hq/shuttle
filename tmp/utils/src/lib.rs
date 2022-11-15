use http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use http_body::{Body, Full};
use hyper::body::Bytes;
use rmps::{Deserializer, Serializer};
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

    // I used Vec<u8> since it can derive serialize/deserialize
    pub body: Vec<u8>,
}

/// Wrap HTTP Request in a struct that can be serialized to and from Rust MessagePack
pub async fn wrap_request<B>(req: Request<B>) -> RequestWrapper
where
    B: Body<Data = Bytes>,
    B::Error: std::fmt::Debug,
{
    let (parts, body) = req.into_parts();

    let body = hyper::body::to_bytes(body).await.unwrap();
    let body = body.iter().cloned().collect::<Vec<u8>>();

    RequestWrapper {
        method: parts.method,
        uri: parts.uri,
        version: parts.version,
        headers: parts.headers,
        body,
    }
}

impl RequestWrapper {
    /// Serialize a RequestWrapper to the Rust MessagePack data format
    pub fn into_rmp(self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf)).unwrap();

        buf
    }

    /// Deserialize a RequestWrapper from the Rust MessagePack data format
    pub fn from_rmp(buf: Vec<u8>) -> Self {
        let mut de = Deserializer::new(buf.as_slice());

        Deserialize::deserialize(&mut de).unwrap()
    }

    /// Consume wrapper and return Request
    pub fn into_request(self) -> Request<Full<Bytes>> {
        let mut request: Request<Full<Bytes>> = Request::builder()
            .method(self.method)
            .version(self.version)
            .uri(self.uri)
            .body(Full::new(self.body.into()))
            .unwrap();

        request.headers_mut().extend(self.headers.into_iter());

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

    // I used Vec<u8> since it can derive serialize/deserialize
    pub body: Vec<u8>,
}

/// Wrap HTTP Response in a struct that can be serialized to and from Rust MessagePack
pub async fn wrap_response<B>(res: Response<B>) -> ResponseWrapper
where
    B: Body<Data = Bytes>,
    B::Error: std::fmt::Debug,
{
    let (parts, body) = res.into_parts();

    let body = hyper::body::to_bytes(body).await.unwrap();
    let body = body.iter().cloned().collect::<Vec<u8>>();

    ResponseWrapper {
        status: parts.status,
        version: parts.version,
        headers: parts.headers,
        body,
    }
}

impl ResponseWrapper {
    /// Serialize a ResponseWrapper into the Rust MessagePack data format
    pub fn into_rmp(self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf)).unwrap();

        buf
    }

    /// Deserialize a ResponseWrapper from the Rust MessagePack data format
    pub fn from_rmp(buf: Vec<u8>) -> Self {
        let mut de = Deserializer::new(buf.as_slice());

        Deserialize::deserialize(&mut de).unwrap()
    }

    /// Consume wrapper and return Response
    pub fn into_response(self) -> Response<hyper::Body> {
        let mut response = Response::builder()
            .status(self.status)
            .version(self.version);
        response
            .headers_mut()
            .unwrap()
            .extend(self.headers.into_iter());

        response.body(hyper::Body::from(self.body)).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use futures_executor::block_on;
    use http::HeaderValue;

    #[test]
    fn request_roundtrip() {
        let request: Request<Full<Bytes>> = Request::builder()
            .method(Method::PUT)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("request"))
            .uri(format!("https://axum-wasm.example/hello"))
            .body(Full::new(Bytes::from_static(b"request body")))
            .unwrap();

        let rmp = block_on(wrap_request(request)).into_rmp();

        let back = RequestWrapper::from_rmp(rmp);

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
        assert_eq!(std::str::from_utf8(&back.body).unwrap(), "request body");
    }

    #[test]
    fn response_roundtrip() {
        let response: Response<Full<Bytes>> = Response::builder()
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("response"))
            .status(StatusCode::NOT_MODIFIED)
            .body(Full::new(Bytes::from_static(b"response body")))
            .unwrap();

        let rmp = block_on(wrap_response(response)).into_rmp();

        let back = ResponseWrapper::from_rmp(rmp);

        assert_eq!(
            back.headers.get("test").unwrap(),
            HeaderValue::from_static("response")
        );
        assert_eq!(back.status, StatusCode::NOT_MODIFIED);
        assert_eq!(back.version, Version::HTTP_11);
        assert_eq!(std::str::from_utf8(&back.body).unwrap(), "response body");
    }
}
