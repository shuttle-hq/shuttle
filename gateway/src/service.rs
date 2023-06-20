use fqdn::{Fqdn, FQDN};
use hyper::client::connect::dns::GaiResolver;
use hyper::client::HttpConnector;
use hyper::Client;
use hyper_reverse_proxy::ReverseProxy;
use instant_acme::{AccountCredentials, ChallengeType};
use once_cell::sync::Lazy;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePool;
use sqlx::{query, Error as SqlxError, Row};
use std::io::Cursor;
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, warn};
use x509_parser::nom::AsBytes;
use x509_parser::parse_x509_certificate;
use x509_parser::prelude::parse_x509_pem;
use x509_parser::time::ASN1Time;

use crate::acme::{AccountWrapper, AcmeClient, CustomDomain};
use crate::tls::{ChainAndPrivateKey, GatewayCertResolver, RENEWAL_VALIDITY_THRESHOLD_IN_DAYS};
use crate::{Error, ErrorKind, ProjectDetails, ProjectName};

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");
static PROXY_CLIENT: Lazy<ReverseProxy<HttpConnector<GaiResolver>>> =
    Lazy::new(|| ReverseProxy::new(Client::new()));

impl From<SqlxError> for Error {
    fn from(err: SqlxError) -> Self {
        debug!("internal SQLx error: {err}");
        Self::source(ErrorKind::Internal, err)
    }
}

pub struct GatewayService {
    db: SqlitePool,
    state_location: PathBuf,
    proxy_fqdn: String,
}

impl GatewayService {
    /// Initialize `GatewayService` and its required dependencies.
    ///
    /// * `args` - The [`Args`] with which the service was
    /// started. Will be passed as [`Context`] to workers and state.
    pub async fn init(db: SqlitePool, state_location: PathBuf, proxy_fqdn: String) -> Self {
        Self {
            db,
            state_location,
            proxy_fqdn,
        }
    }

    pub async fn control_key_from_project_name(
        &self,
        project_name: &ProjectName,
    ) -> Result<String, Error> {
        let control_key = query("SELECT initial_key FROM projects WHERE project_name = ?1")
            .bind(project_name)
            .fetch_optional(&self.db)
            .await?
            .map(|row| row.try_get("initial_key").unwrap())
            .ok_or_else(|| Error::from(ErrorKind::ProjectNotFound))?;
        Ok(control_key)
    }

    pub async fn create_custom_domain(
        &self,
        project_name: &ProjectName,
        fqdn: &Fqdn,
        certs: &str,
        private_key: &str,
    ) -> Result<(), Error> {
        query("INSERT OR REPLACE INTO custom_domains (fqdn, project_name, certificate, private_key) VALUES (?1, ?2, ?3, ?4)")
            .bind(fqdn.to_string())
            .bind(project_name)
            .bind(certs)
            .bind(private_key)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    pub async fn iter_custom_domains(&self) -> Result<impl Iterator<Item = CustomDomain>, Error> {
        query("SELECT fqdn, project_name, certificate, private_key FROM custom_domains")
            .fetch_all(&self.db)
            .await
            .map(|res| {
                res.into_iter().map(|row| CustomDomain {
                    fqdn: row.get::<&str, _>("fqdn").parse().unwrap(),
                    project_name: row.try_get("project_name").unwrap(),
                    certificate: row.get("certificate"),
                    private_key: row.get("private_key"),
                })
            })
            .map_err(|_| Error::from_kind(ErrorKind::Internal))
    }

    pub async fn find_custom_domain_for_project(
        &self,
        project_name: &ProjectName,
    ) -> Result<CustomDomain, Error> {
        let custom_domain = query(
            "SELECT fqdn, project_name, certificate, private_key FROM custom_domains WHERE project_name = ?1",
        )
        .bind(project_name.to_string())
        .fetch_optional(&self.db)
        .await?
        .map(|row| CustomDomain {
            fqdn: row.get::<&str, _>("fqdn").parse().unwrap(),
            project_name: row.try_get("project_name").unwrap(),
            certificate: row.get("certificate"),
            private_key: row.get("private_key"),
        })
        .ok_or_else(|| Error::from(ErrorKind::CustomDomainNotFound))?;
        Ok(custom_domain)
    }

    pub async fn project_details_for_custom_domain(
        &self,
        fqdn: &Fqdn,
    ) -> Result<CustomDomain, Error> {
        let custom_domain = query(
            "SELECT fqdn, project_name, certificate, private_key FROM custom_domains WHERE fqdn = ?1",
        )
        .bind(fqdn.to_string())
        .fetch_optional(&self.db)
        .await?
        .map(|row| CustomDomain {
            fqdn: row.get::<&str, _>("fqdn").parse().unwrap(),
            project_name: row.try_get("project_name").unwrap(),
            certificate: row.get("certificate"),
            private_key: row.get("private_key"),
        })
        .ok_or_else(|| Error::from(ErrorKind::CustomDomainNotFound))?;
        Ok(custom_domain)
    }

    pub async fn iter_projects_detailed(
        &self,
    ) -> Result<impl Iterator<Item = ProjectDetails>, Error> {
        let iter = query("SELECT project_name, account_name FROM projects")
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| ProjectDetails {
                project_name: row.try_get("project_name").unwrap(),
                account_name: row.try_get("account_name").unwrap(),
            });
        Ok(iter)
    }

    /// Returns the current certificate as a pair of the chain and private key.
    /// If the pair doesn't exist for a specific project, create both the certificate
    /// and the custom domain it will represent.
    pub async fn create_custom_domain_certificate(
        &self,
        fqdn: &Fqdn,
        acme_client: &AcmeClient,
        project_name: &ProjectName,
        creds: AccountCredentials<'_>,
    ) -> Result<(String, String), Error> {
        match self.project_details_for_custom_domain(fqdn).await {
            Ok(CustomDomain {
                certificate,
                private_key,
                ..
            }) => Ok((certificate, private_key)),
            Err(err) if err.kind() == ErrorKind::CustomDomainNotFound => {
                let (certs, private_key) = acme_client
                    .create_certificate(&fqdn.to_string(), ChallengeType::Http01, creds)
                    .await?;
                self.create_custom_domain(project_name, fqdn, &certs, &private_key)
                    .await?;
                Ok((certs, private_key))
            }
            Err(err) => Err(err),
        }
    }

    async fn create_certificate<'a>(
        &self,
        acme: &AcmeClient,
        creds: AccountCredentials<'a>,
    ) -> ChainAndPrivateKey {
        let public: FQDN = self.proxy_fqdn.parse().unwrap();
        let identifier = format!("*.{public}");

        // Use ::Dns01 challenge because that's the only supported
        // challenge type for wildcard domains.
        let (chain, private_key) = acme
            .create_certificate(&identifier, ChallengeType::Dns01, creds)
            .await
            .unwrap();

        let mut buf = Vec::new();
        buf.extend(chain.as_bytes());
        buf.extend(private_key.as_bytes());

        ChainAndPrivateKey::parse_pem(Cursor::new(buf)).expect("Malformed PEM buffer.")
    }

    /// Fetch the gateway certificate from the state location.
    /// If not existent, create the gateway certificate and save it to the
    /// gateway state.
    pub async fn fetch_certificate(
        &self,
        acme: &AcmeClient,
        creds: AccountCredentials<'_>,
    ) -> ChainAndPrivateKey {
        let tls_path = self.state_location.join("ssl.pem");
        match ChainAndPrivateKey::load_pem(&tls_path) {
            Ok(valid) => valid,
            Err(_) => {
                warn!(
                    "no valid certificate found at {}, creating one...",
                    tls_path.display()
                );

                let certs = self.create_certificate(acme, creds).await;
                certs.clone().save_pem(&tls_path).unwrap();
                certs
            }
        }
    }

    /// Renew the gateway certificate if there less than 30 days until the current
    /// certificate expiration.
    pub(crate) async fn renew_certificate(
        &self,
        acme: &AcmeClient,
        resolver: Arc<GatewayCertResolver>,
        creds: AccountCredentials<'_>,
    ) {
        let account = AccountWrapper::from(creds).0;
        let certs = self.fetch_certificate(acme, account.credentials()).await;
        // Safe to unwrap because a 'ChainAndPrivateKey' is built from a PEM.
        let chain_and_pk = certs.into_pem().unwrap();

        let (_, pem) = parse_x509_pem(chain_and_pk.as_bytes())
            .unwrap_or_else(|_| panic!("Malformed existing PEM certificate for the gateway."));
        let (_, x509_cert) = parse_x509_certificate(pem.contents.as_bytes())
            .unwrap_or_else(|_| panic!("Malformed existing X509 certificate for the gateway."));

        // We compute the difference between the certificate expiry date and current timestamp because we want to trigger the
        // gateway certificate renewal only during it's last 30 days of validity or if the certificate is expired.
        let diff = x509_cert.validity().not_after.sub(ASN1Time::now());

        // Renew only when the difference is `None` (meaning certificate expired) or we're within the last 30 days of validity.
        if diff.is_none()
            || diff
                .expect("to be Some given we checked for None previously")
                .whole_days()
                <= RENEWAL_VALIDITY_THRESHOLD_IN_DAYS
        {
            let tls_path = self.state_location.join("ssl.pem");
            let certs = self.create_certificate(acme, account.credentials()).await;
            resolver
                .serve_default_der(certs.clone())
                .await
                .expect("Failed to serve the default certs");
            certs
                .save_pem(&tls_path)
                .expect("to save the certificate locally");
        }
    }

    pub fn credentials(&self) -> AccountCredentials<'_> {
        let creds_path = self.state_location.join("acme.json");
        if !creds_path.exists() {
            panic!(
                "no ACME credentials found at {}, cannot continue with certificate creation",
                creds_path.display()
            );
        }

        serde_json::from_reader(std::fs::File::open(creds_path).expect("Invalid credentials path"))
            .expect("Can not parse admin credentials from path")
    }
}

#[cfg(test)]
pub mod tests {
    use fqdn::FQDN;

    use super::*;

    use crate::{
        tests::{assert_err_kind, World},
        AccountName,
    };

    #[tokio::test]
    async fn service_create_find_custom_domain() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc =
            Arc::new(GatewayService::init(world.pool(), "".into(), world.fqdn().to_string()).await);

        let account: AccountName = "neo".parse().unwrap();
        let project_name: ProjectName = "matrix".parse().unwrap();
        let domain: FQDN = "neo.the.matrix".parse().unwrap();
        let certificate = "dummy certificate";
        let private_key = "dummy private key";

        assert_err_kind!(
            svc.project_details_for_custom_domain(&domain).await,
            ErrorKind::CustomDomainNotFound
        );

        svc.create_custom_domain(&project_name, &domain, certificate, private_key)
            .await
            .unwrap();

        let custom_domain = svc
            .project_details_for_custom_domain(&domain)
            .await
            .unwrap();

        assert_eq!(custom_domain.project_name, project_name);
        assert_eq!(custom_domain.certificate, certificate);
        assert_eq!(custom_domain.private_key, private_key);

        // Should auto replace the domain details
        let certificate = "dummy certificate update";
        let private_key = "dummy private key update";

        svc.create_custom_domain(&project_name, &domain, certificate, private_key)
            .await
            .unwrap();

        let custom_domain = svc
            .project_details_for_custom_domain(&domain)
            .await
            .unwrap();

        assert_eq!(custom_domain.project_name, project_name);
        assert_eq!(custom_domain.certificate, certificate);
        assert_eq!(custom_domain.private_key, private_key);

        Ok(())
    }

    #[tokio::test]
    async fn service_create_custom_domain() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc =
            Arc::new(GatewayService::init(world.pool(), "".into(), world.fqdn().to_string()).await);

        let account: AccountName = "neo".parse().unwrap();
        let project_name: ProjectName = "matrix".parse().unwrap();
        let domain: FQDN = "neo.the.matrix".parse().unwrap();
        let certificate = "dummy certificate";
        let private_key = "dummy private key";

        assert_err_kind!(
            svc.project_details_for_custom_domain(&domain).await,
            ErrorKind::CustomDomainNotFound
        );

        svc.create_custom_domain(&project_name, &domain, certificate, private_key)
            .await
            .unwrap();

        Ok(())
    }
}
