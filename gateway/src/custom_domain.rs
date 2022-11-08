use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomDomain {
    // TODO: update custom domain states, these are just placeholders for now
    Creating,
    Verifying,
    IssuingCertificate,
    Ready,
    Errored,
}
