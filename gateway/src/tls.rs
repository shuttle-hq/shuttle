use std::collections::HashMap;
use std::io::BufReader;
use std::sync::Arc;

use axum_server::accept::DefaultAcceptor;
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};
use futures::executor::block_on;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::{self, CertifiedKey};
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::Item;
use shuttle_common::models::error::ErrorKind;
use tokio::runtime::Handle;
use tokio::sync::RwLock;

use crate::service::GatewayService;
use crate::{Error, Fqdn};

pub fn parse_pem(certs: &[u8], key: &[u8]) -> Result<(Vec<Certificate>, PrivateKey), Error> {
    let certs = rustls_pemfile::read_all(&mut BufReader::new(certs))
        .map_err(|_| Error::from_kind(ErrorKind::Internal))
        .and_then(|items| {
            items
                .into_iter()
                .map(|item| match item {
                    Item::X509Certificate(cert) => Ok(Certificate(cert)),
                    _ => Err(Error::from_kind(ErrorKind::Internal)),
                })
                .collect()
        })?;

    let private_key = match rustls_pemfile::read_one(&mut BufReader::new(key)).unwrap() {
        Some(Item::RSAKey(key)) | Some(Item::PKCS8Key(key)) | Some(Item::ECKey(key)) => {
            Ok(PrivateKey(key))
        }
        _ => Err(Error::from_kind(ErrorKind::Internal)),
    }?;

    Ok((certs, private_key))
}

pub struct GatewayCertResolver {
    keys: RwLock<HashMap<String, Arc<CertifiedKey>>>,
}

impl GatewayCertResolver {
    pub fn new() -> Self {
        Self {
            keys: RwLock::new(HashMap::default()),
        }
    }

    /// Get the loaded [CertifiedKey] associated with the given
    /// domain.
    pub async fn get(&self, sni: &str) -> Option<Arc<CertifiedKey>> {
        self.keys.read().await.get(sni).map(Arc::clone)
    }

    /// Load a new certificate chain and private key to serve when
    /// receiving incoming TLS connections for the given domain.
    pub async fn serve_der(
        &self,
        fqdn: Fqdn,
        certs: Vec<Certificate>,
        key: PrivateKey,
    ) -> Result<(), Error> {
        let signing_key =
            sign::any_supported_type(&key).map_err(|_| Error::from_kind(ErrorKind::Internal))?;
        let certified_key = CertifiedKey::new(certs, signing_key);
        self.keys
            .write()
            .await
            .insert(fqdn.to_string(), Arc::new(certified_key));
        Ok(())
    }

    /// Same as [GatewayCertResolver::serve_der] but assuming the
    /// certificate and keys are provided as PEM files which have to
    /// be parsed.
    pub async fn serve_pem(&self, fqdn: Fqdn, certs: &[u8], key: &[u8]) -> Result<(), Error> {
        let (certs, private_key) = parse_pem(certs, key)?;
        self.serve_der(fqdn, certs, private_key).await
    }
}

impl ResolvesServerCert for GatewayCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello.server_name()?;
        let handle = Handle::current();
        handle.enter();
        block_on(self.get(sni))
    }
}

pub fn make_tls_acceptor() -> (Arc<GatewayCertResolver>, RustlsAcceptor<DefaultAcceptor>) {
    let resolver = Arc::new(GatewayCertResolver::new());

    let mut server_config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_cert_resolver(Arc::clone(&resolver) as Arc<dyn ResolvesServerCert>);
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let rustls_config = RustlsConfig::from_config(Arc::new(server_config));

    (resolver, RustlsAcceptor::new(rustls_config))
}
