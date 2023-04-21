use axum::http::Request;
use shuttle_common::claims::{Claim, Scope};

/// This middleware sets an admin token in the claim extension of every request, so we can
/// develop deployer locally without starting it with the gateway and without proxying commands
/// through gateway.
pub async fn set_admin_claim<B>(mut request: Request<B>) -> Request<B> {
    let claim = Claim::new("admin".to_string(), Scope::admin());

    request.extensions_mut().insert::<Claim>(claim);

    request
}
