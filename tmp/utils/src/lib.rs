use http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use rmps::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

extern crate rmp_serde as rmps;

// todo: add extensions
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

impl<B> From<Request<B>> for RequestWrapper {
    fn from(req: Request<B>) -> Self {
        let (parts, _) = req.into_parts();

        Self {
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

    /// Deserialize a RequestWrapper from the Rust MessagePack data format
    pub fn from_rmp(buf: Vec<u8>) -> Self {
        let mut de = Deserializer::new(buf.as_slice());

        Deserialize::deserialize(&mut de).unwrap()
    }
}

// todo: add extensions
#[derive(Serialize, Deserialize, Debug)]
pub struct ResponseWrapper {
    #[serde(with = "http_serde::status_code")]
    pub status: StatusCode,

    #[serde(with = "http_serde::version")]
    pub version: Version,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
}

impl<B> From<Response<B>> for ResponseWrapper {
    fn from(res: Response<B>) -> Self {
        let (parts, _) = res.into_parts();

        Self {
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

    /// Deserialize a ResponseWrapper from the Rust MessagePack data format
    pub fn from_rmp(buf: Vec<u8>) -> Self {
        let mut de = Deserializer::new(buf.as_slice());

        Deserialize::deserialize(&mut de).unwrap()
    }
}

#[cfg(test)]
mod test {
    use http::HeaderValue;

    use super::*;

    #[test]
    fn request_roundtrip() {
        let request: Request<String> = Request::builder()
            .method(Method::PUT)
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("request"))
            .uri(format!("https://axum-wasm.example/hello"))
            .body("Some body".to_string())
            .unwrap();

        let rmp = RequestWrapper::from(request).into_rmp();

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
    }

    #[test]
    fn response_roundtrip() {
        let response: Response<String> = Response::builder()
            .version(Version::HTTP_11)
            .header("test", HeaderValue::from_static("response"))
            .status(StatusCode::NOT_MODIFIED)
            .body("Some body".to_string())
            .unwrap();

        let rmp = ResponseWrapper::from(response).into_rmp();

        let back = ResponseWrapper::from_rmp(rmp);

        assert_eq!(
            back.headers.get("test").unwrap(),
            HeaderValue::from_static("response")
        );
        assert_eq!(back.status, StatusCode::NOT_MODIFIED);
        assert_eq!(back.version, Version::HTTP_11);
    }
}
