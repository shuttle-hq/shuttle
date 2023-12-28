use http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version};
use rmps::Serializer;
use serde::{Deserialize, Serialize};

extern crate rmp_serde as rmps;

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
    pub fn into_rmp(self) -> Result<Vec<u8>, rmps::encode::Error> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf))?;

        Ok(buf)
    }

    /// Consume the wrapper and return a request builder with `Parts` set
    pub fn into_request_builder(self) -> http::request::Builder {
        let mut request = Request::builder()
            .method(self.method)
            .version(self.version)
            .uri(self.uri);

        request
            .headers_mut()
            .unwrap() // Safe to unwrap as we just made the builder
            .extend(self.headers);

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
    pub fn into_rmp(self) -> Result<Vec<u8>, rmps::encode::Error> {
        let mut buf = Vec::new();
        self.serialize(&mut Serializer::new(&mut buf))?;

        Ok(buf)
    }

    /// Consume the wrapper and return a response builder with `Parts` set
    pub fn into_response_builder(self) -> http::response::Builder {
        let mut response = Response::builder()
            .status(self.status)
            .version(self.version);

        response
            .headers_mut()
            .unwrap() // Safe to unwrap since we just made the builder
            .extend(self.headers);

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use hyper::body::Body;

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
        let rmp = RequestWrapper::from(parts).into_rmp().unwrap();

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
        let rmp = ResponseWrapper::from(parts).into_rmp().unwrap();

        let back: ResponseWrapper = rmps::from_slice(&rmp).unwrap();

        assert_eq!(
            back.headers.get("test").unwrap(),
            HeaderValue::from_static("response")
        );
        assert_eq!(back.status, StatusCode::NOT_MODIFIED);
        assert_eq!(back.version, Version::HTTP_11);
    }
}
