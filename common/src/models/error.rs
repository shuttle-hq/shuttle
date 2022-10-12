use std::fmt::{Display, Formatter};

use comfy_table::Color;
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message.to_string().with(Color::Red))
    }
}

impl std::error::Error for ApiError {}
