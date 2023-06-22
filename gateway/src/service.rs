use fqdn::{Fqdn, FQDN};
use instant_acme::{AccountCredentials, ChallengeType};
use std::io::Cursor;
use std::ops::Sub;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::warn;
use x509_parser::nom::AsBytes;
use x509_parser::parse_x509_certificate;
use x509_parser::prelude::parse_x509_pem;
use x509_parser::time::ASN1Time;

use crate::acme::{AccountWrapper, AcmeClient, CustomDomain};
use crate::dal::Dal;
use crate::tls::{ChainAndPrivateKey, GatewayCertResolver, RENEWAL_VALIDITY_THRESHOLD_IN_DAYS};
use crate::{AccountName, Error, ErrorKind, ProjectDetails, ProjectName};

#[derive(Clone)]
pub struct GatewayService<D: Dal> {
    dal: D,
    state_location: PathBuf,
    proxy_fqdn: FQDN,
}

impl<D> GatewayService<D>
where
    D: Dal,
{
    /// Initialize `GatewayService` and its required dependencies.
    pub async fn init(dal: D, state_location: PathBuf, proxy_fqdn: FQDN) -> Self {
        Self {
            dal,
            state_location,
            proxy_fqdn,
        }
    }

    pub async fn find_project(&self, project_name: &ProjectName) -> Result<ProjectName, Error> {
        let result = self.dal.get_project(project_name).await?;

        Ok(result)
    }

    /// Fetch an iterator over all projects.
    pub async fn iter_projects(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = ProjectDetails>, Error> {
        let iter = self.dal.get_all_projects().await?.into_iter();

        Ok(iter)
    }

    pub async fn iter_user_projects(
        &self,
        account_name: &AccountName,
    ) -> Result<impl Iterator<Item = ProjectName>, Error> {
        let iter = self
            .dal
            .get_user_projects(account_name)
            .await
            .map(|projects| projects.into_iter())?;

        Ok(iter)
    }

    pub async fn iter_user_projects_paginated(
        &self,
        account_name: &AccountName,
        offset: u32,
        limit: u32,
    ) -> Result<impl Iterator<Item = ProjectName>, Error> {
        let iter = self
            .dal
            .get_user_projects_paginated(account_name, offset, limit)
            .await
            .map(|projects| projects.into_iter())?;

        Ok(iter)
    }

    pub async fn create_custom_domain(
        &self,
        project_name: &ProjectName,
        fqdn: &Fqdn,
        certs: &str,
        private_key: &str,
    ) -> Result<(), Error> {
        self.dal
            .create_custom_domain(project_name, fqdn, certs, private_key)
            .await?;

        Ok(())
    }

    pub async fn iter_custom_domains(&self) -> Result<impl Iterator<Item = CustomDomain>, Error> {
        let result = self
            .dal
            .get_custom_domains()
            .await
            .map(|domains| domains.into_iter())?;

        Ok(result)
    }

    pub async fn find_custom_domain_for_project(
        &self,
        project_name: &ProjectName,
    ) -> Result<CustomDomain, Error> {
        let result = self
            .dal
            .find_custom_domain_for_project(project_name)
            .await?;

        Ok(result)
    }

    pub async fn project_details_for_custom_domain(
        &self,
        fqdn: &Fqdn,
    ) -> Result<CustomDomain, Error> {
        let result = self.dal.project_details_for_custom_domain(fqdn).await?;

        Ok(result)
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
        let public = self.proxy_fqdn.clone();
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

    use crate::tests::{assert_err_kind, World};

    #[tokio::test]
    async fn service_create_find_custom_domain() -> anyhow::Result<()> {
        let world = World::new().await;
        let svc = Arc::new(GatewayService::init(world.pool(), "".into(), world.fqdn()).await);

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
        let svc = Arc::new(GatewayService::init(world.pool(), "".into(), world.fqdn()).await);

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
