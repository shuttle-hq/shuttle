use headers::{Header, HeaderName};
use http::HeaderValue;

pub static X_SHUTTLE_ADMIN_SECRET: HeaderName = HeaderName::from_static("x-shuttle-admin-secret");

/// Typed header for sending admin secrets to Shuttle components
pub struct XShuttleAdminSecret(pub String);

impl Header for XShuttleAdminSecret {
    fn name() -> &'static HeaderName {
        &X_SHUTTLE_ADMIN_SECRET
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i http::HeaderValue>,
    {
        let value = values
            .next()
            .ok_or_else(headers::Error::invalid)?
            .to_str()
            .map_err(|_| headers::Error::invalid())?
            .to_string();

        Ok(Self(value))
    }

    fn encode<E: Extend<http::HeaderValue>>(&self, values: &mut E) {
        if let Ok(value) = HeaderValue::from_str(&self.0) {
            values.extend(std::iter::once(value));
        }
    }
}

/// Used by deployers <=0.38.0. Can be removed when those are no longer supported
pub static X_SHUTTLE_PROJECT: HeaderName = HeaderName::from_static("x-shuttle-project");

pub struct XShuttleProject(pub String);

impl Header for XShuttleProject {
    fn name() -> &'static HeaderName {
        &X_SHUTTLE_PROJECT
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i HeaderValue>,
    {
        let value = values
            .next()
            .ok_or_else(headers::Error::invalid)?
            .to_str()
            .map_err(|_| headers::Error::invalid())?
            .to_string();

        Ok(Self(value))
    }

    fn encode<E: Extend<http::HeaderValue>>(&self, values: &mut E) {
        if let Ok(value) = HeaderValue::from_str(self.0.as_str()) {
            values.extend(std::iter::once(value));
        }
    }
}
