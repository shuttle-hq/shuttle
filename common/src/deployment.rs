use std::str::FromStr;

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Display, Serialize, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[strum(ascii_case_insensitive)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "openapi", schema(as = shuttle_common::deployment::State))]
pub enum State {
    Queued,
    Building,
    Built,
    Loading,
    Running,
    Completed,
    Stopped,
    Crashed,
    Unknown,
}

/// This which environment is this deployment taking place
#[derive(Clone, Copy, Debug)]
pub enum Environment {
    Local,
    Production,
}

impl FromStr for Environment {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Environment::Local),
            "production" => Ok(Environment::Production),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::deployment::State;

    #[test]
    fn test_state_deser() {
        assert_eq!(State::Queued, State::from_str("Queued").unwrap());
        assert_eq!(State::Unknown, State::from_str("unKnown").unwrap());
        assert_eq!(State::Built, State::from_str("built").unwrap());
    }
}
