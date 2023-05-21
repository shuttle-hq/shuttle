use http::Method;
use oci_spec::image::*;

use url::Url;

use super::{super::error::*, super::image::digest::Digest, Name, Reference, StoredAuth};

/// A client for `/v2/<name>/` API endpoint
pub struct Client {
    agent: reqwest::Client,
    /// URL to registry server
    url: Url,
    /// Name of repository
    name: Name,
    /// Loaded authentication info from filesystem
    auth: StoredAuth,
    /// Cached token
    token: Option<String>,
}

impl Client {
    pub fn new(url: Url, name: Name) -> Result<Self> {
        let auth = StoredAuth::load_all()?;
        Ok(Client {
            agent: reqwest::Client::new(),
            url,
            name,
            auth,
            token: None,
        })
    }

    #[async_recursion::async_recursion]
    async fn post(&mut self, url: Url) -> Result<reqwest::Response> {
        // This is a guard that skips authorization for a localhost container registry.
        if url.as_str().contains("localhost") {
            let req_builder = self.agent.request(Method::POST, url);
            return req_builder
                .send()
                .await
                .map_err(|err| super::super::error::Error::Reqwest(err.to_string()));
        }

        // If we already have the token just continue with the request.
        if let Some(token) = &self.token {
            let req_builder = self.agent.request(Method::POST, url);
            return req_builder
                .header(http::header::AUTHORIZATION, format!("Bearer {}", token))
                .send()
                .await
                .map_err(|err| Error::Reqwest(err.to_string()));
        }

        // Try getting the token. A response can look like the one below:
        //
        // ```text
        // 401 Unauthorized
        // WWW-Authenticate: <scheme> realm="<realm>", ..."
        // Content-Length: <length>
        // Content-Type: application/json

        // {
        // 	"errors": [
        // 	    {
        //             "code": <error code>,
        //             "message": "<error message>",
        //             "detail": ...
        //         },
        //         ...
        //     ]
        // }
        // ```
        let req_builder = self.agent.request(Method::POST, url.clone());
        let www_auth = match req_builder.send().await {
            Ok(res) => {
                if res.status().as_u16() == 401 {
                    if res.headers().contains_key(http::header::WWW_AUTHENTICATE) {
                        Ok(res
                            .headers()
                            .get(http::header::WWW_AUTHENTICATE)
                            .expect("to get a value for the WWW-AUTHENTICATE header")
                            .to_str()
                            .expect("to have a valid response from server")
                            .to_string())
                    } else {
                        Err(Error::Reqwest(
                            res.text().await.expect("to get a body response"),
                        ))
                    }
                } else {
                    Err(Error::Reqwest(
                        res.text().await.expect("to get a body response"),
                    ))
                }
            }
            Err(err) => Err(Error::Reqwest(err.to_string())),
        }?;
        let challenge = super::AuthChallenge::from_header(&www_auth)?;
        self.token = Some(self.auth.challenge(&challenge).await?);
        self.post(url).await
    }

    /// Push manifest to registry
    ///
    /// ```text
    /// PUT /v2/<name>/manifests/<reference>
    /// ```
    ///
    /// Manifest must be pushed after blobs are updated.
    ///
    /// See [corresponding OCI distribution spec document](https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pushing-manifests) for detail.
    pub async fn push_manifest(
        &self,
        reference: &Reference,
        manifest: &ImageManifest,
    ) -> Result<reqwest::Response> {
        let mut buf = Vec::new();
        manifest.to_writer(&mut buf)?;
        let url = self
            .url
            .join(&format!("/v2/{}/manifests/{}", self.name, reference))?;
        let mut req_builder = self.agent.put(url).header(
            http::header::CONTENT_TYPE,
            MediaType::ImageManifest.to_string(),
        );

        if let Some(token) = self.token.as_ref() {
            // Authorization must be done while blobs push
            req_builder =
                req_builder.header(http::header::AUTHORIZATION, format!("Bearer {}", token));
        }

        req_builder
            .body(buf)
            .send()
            .await
            .map_err(|err| Error::Reqwest(err.to_string()))
    }

    /// Push blob to registry
    ///
    /// ```text
    /// POST /v2/<name>/blobs/uploads/
    /// ```
    ///
    /// and following `PUT` to URL obtained by `POST`.
    ///
    /// See [corresponding OCI distribution spec document](https://github.com/opencontainers/distribution-spec/blob/main/spec.md#pushing-manifests) for detail.
    pub async fn push_blob(&mut self, blob: Vec<u8>) -> Result<Url> {
        let url = self
            .url
            .join(&format!("/v2/{}/blobs/uploads/", self.name))?;
        let res = self.post(url.clone()).await?;
        let loc = res
            .headers()
            .get(http::header::LOCATION)
            .expect("Location header missing from the OCI registry response")
            .to_str()
            .expect("to get a location str");
        let url = Url::parse(loc).or_else(|_| self.url.join(loc))?;

        let digest = Digest::from_buf_sha256(&blob);
        let mut req_builder = self
            .agent
            .put(url)
            .query(&[("digest", &digest.to_string())])
            .header(http::header::CONTENT_LENGTH, &blob.len().to_string())
            .header(http::header::CONTENT_TYPE, "application/octet-stream");

        if let Some(token) = self.token.as_ref() {
            // Authorization must be done while the first POST
            req_builder =
                req_builder.header(http::header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let res = req_builder
            .body(blob)
            .send()
            .await
            .map_err(|err| Error::Reqwest(err.to_string()))?;

        let loc = res
            .headers()
            .get(http::header::LOCATION)
            .expect("Location header missing from the OCI registry response");
        Ok(Url::parse(loc.to_str().expect("to parse an url"))
            .or_else(|_| self.url.join(loc.to_str().expect("to join with an url")))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    //
    // Following tests need a registry server. See test/fixture.sh for setting one up.
    // These tests are ignored by default.
    //

    // TODO: add the necessary .circleci setup to test this.
    fn test_url() -> Url {
        Url::parse("http://localhost:5000").unwrap()
    }
    fn test_name() -> Name {
        Name::new("test_repo").unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn push_blob() -> Result<()> {
        let mut client = Client::new(test_url(), test_name())?;
        let url = client.push_blob(vec![1, 2, 3, 4]).await?;
        dbg!(url);
        Ok(())
    }
}
