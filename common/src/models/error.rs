use std::fmt::{Display, Formatter};

use comfy_table::Color;
use crossterm::style::Stylize;
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiError {
    pub message: String,
    pub status_code: u16,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\nMessage: {}",
            StatusCode::from_u16(self.status_code)
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
                .to_string()
                .bold(),
            self.message.to_string().with(Color::Red)
        )
    }
}

impl std::error::Error for ApiError {}
