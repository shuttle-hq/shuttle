use oci_spec::{distribution::*, image::*};
use tracing::info;
use url::Url;

use super::{super::digest::Digest, super::error::*, Name, Reference, StoredAuth};

/// A client for `/v2/<name>/` API endpoint
pub struct Client {
    agent: ureq::Agent,
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
            agent: ureq::Agent::new(),
            url,
            name,
            auth,
            token: None,
        })
    }

    fn call(&mut self, req: ureq::Request) -> Result<ureq::Response> {
        if req.url().contains("localhost") {
            return req
                .call()
                .map_err(|err| super::super::error::Error::Registry(err.to_string()));
        }

        // Try get token
        if let Some(token) = &self.token {
            return Ok(req
                .set("Authorization", &format!("Bearer {}", token))
                .call()?);
        }

        let try_req = req.clone();
        let www_auth = match try_req.call() {
            Ok(res) => return Ok(res),
            Err(ureq::Error::Status(status, res)) => {
                if status == 401 && res.has("www-authenticate") {
                    res.header("www-authenticate").unwrap().to_string()
                } else {
                    let err = res.into_json::<ErrorResponse>()?;
                    return Err(Error::Registry(err.to_string()));
                }
            }
            Err(ureq::Error::Transport(e)) => return Err(Error::Network(e.to_string())),
        };
        let challenge = super::AuthChallenge::from_header(&www_auth)?;
        self.token = Some(self.auth.challenge(&challenge)?);
        self.call(req)
    }

    fn put(&self, url: &Url) -> ureq::Request {
        self.agent.put(url.as_str())
    }

    fn post(&self, url: &Url) -> ureq::Request {
        self.agent.post(url.as_str())
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
    pub fn push_manifest(&self, reference: &Reference, manifest: &ImageManifest) -> Result<()> {
        let mut buf = Vec::new();
        manifest.to_writer(&mut buf)?;
        let url = self
            .url
            .join(&format!("/v2/{}/manifests/{}", self.name, reference))?;
        let mut req = self
            .put(&url)
            .set("Content-Type", &MediaType::ImageManifest.to_string());
        if let Some(token) = self.token.as_ref() {
            // Authorization must be done while blobs push
            req = req.set("Authorization", &format!("Bearer {}", token));
        }
        let res = req.send_bytes(&buf)?;
        info!("res {}", res.into_string().unwrap());
        Ok(())
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
    pub fn push_blob(&mut self, blob: &[u8]) -> Result<Url> {
        let url = self
            .url
            .join(&format!("/v2/{}/blobs/uploads/", self.name))?;
        let res = self.call(self.post(&url))?;
        let loc = res
            .header("Location")
            .expect("Location header is lacked in OCI registry response");
        let url = Url::parse(loc).or_else(|_| self.url.join(loc))?;

        let digest = Digest::from_buf_sha256(blob);
        let mut req = self
            .put(&url)
            .query("digest", &digest.to_string())
            .set("Content-Length", &blob.len().to_string())
            .set("Content-Type", "application/octet-stream");
        if let Some(token) = self.token.as_ref() {
            // Authorization must be done while the first POST
            req = req.set("Authorization", &format!("Bearer {}", token))
        }
        let res = req.send_bytes(blob)?;
        let loc = res
            .header("Location")
            .expect("Location header is lacked in OCI registry response");
        Ok(Url::parse(loc).or_else(|_| self.url.join(loc))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    //
    // Following tests need registry server. See test/fixture.sh for setting.
    // These tests are ignored by default.
    //

    fn test_url() -> Url {
        Url::parse("http://localhost:5000").unwrap()
    }
    fn test_name() -> Name {
        Name::new("test_repo").unwrap()
    }

    #[test]
    #[ignore]
    fn push_blob() -> Result<()> {
        let mut client = Client::new(test_url(), test_name())?;
        let url = client.push_blob("test string".as_bytes())?;
        dbg!(url);
        Ok(())
    }
}
