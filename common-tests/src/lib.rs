pub mod cargo_shuttle;
pub mod logger;
pub mod permit_pdp;
pub mod postgres;
pub mod provisioner;

use shuttle_common::{
    claims::{Claim, Scope},
    models::user::AccountTier,
};

/// Layer to set JwtScopes on a request.
/// For use in other tests
#[derive(Clone)]
pub struct JwtScopesLayer {
    /// Thes scopes to set
    scopes: Vec<Scope>,
}

impl JwtScopesLayer {
    /// Create a new layer to set scopes on requests
    pub fn new(scopes: Vec<Scope>) -> Self {
        Self { scopes }
    }
}

impl<S> tower::Layer<S> for JwtScopesLayer {
    type Service = JwtScopes<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JwtScopes {
            inner,
            scopes: self.scopes.clone(),
        }
    }
}

/// Middleware to set scopes on a request
#[derive(Clone)]
pub struct JwtScopes<S> {
    inner: S,
    scopes: Vec<Scope>,
}

impl<S> tower::Service<hyper::Request<hyper::Body>> for JwtScopes<S>
where
    S: tower::Service<hyper::Request<hyper::Body>> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: hyper::Request<hyper::Body>) -> Self::Future {
        let mut c = Claim::new(
            "user-1".to_string(),
            self.scopes.clone(),
            Default::default(),
            AccountTier::default(),
        );
        c.token = Some("user-1".to_string());
        req.extensions_mut().insert(c);
        self.inner.call(req)
    }
}

pub trait ClaimTestsExt {
    /// Fill the token of a test key correctly
    fn fill_token(self) -> Self;
}

impl ClaimTestsExt for Claim {
    fn fill_token(mut self) -> Self {
        self.token = Some(self.sub.clone());

        self
    }
}
