use actix_web::{http::StatusCode, ResponseError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Failed to fetch image from URL: {0}")]
    FetchError(#[from] reqwest::Error),

    #[error("Upstream image fetch failed for url: {url} (status: {status})")]
    FetchStatusError { url: String, status: StatusCode },

    #[error("Failed to decode image: {0}")]
    ImageDecodeError(#[from] image::ImageError),

    #[error("Failed to encode image to {format}: {source}")]
    ImageEncodeError {
        format: String,
        #[source]
        source: image::ImageError,
    },

    #[error("Failed to create image buffer")]
    ImageBufferError,

    #[error("Invalid action parameter: {0}")]
    InvalidActionParam(String),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::FetchStatusError { .. } => StatusCode::BAD_GATEWAY,
            AppError::InvalidActionParam(_) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
