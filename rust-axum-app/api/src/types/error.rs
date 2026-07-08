use aws_sdk_s3::{
    error::SdkError,
    operation::{get_object::GetObjectError, put_object::PutObjectError},
};
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub enum AuthError {
    MissingToken,
    InvalidToken,
    TokenCreation,
    TokenExpired,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::MissingToken => (StatusCode::BAD_REQUEST, "Missing authorization"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::TokenCreation => (StatusCode::BAD_REQUEST, "Error during token creation"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token Expired"),
        };

        let body = Json(serde_json::json!({
      "error": error_message,    }));

        (status, body).into_response()
    }
}

pub enum UploadError {
    UploadConflict,
    OtherError,
}

impl IntoResponse for UploadError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            UploadError::UploadConflict => {
                (StatusCode::CONFLICT, "Error Unique violation constraint")
            }
            UploadError::OtherError => (StatusCode::INTERNAL_SERVER_ERROR, "Other Error Occurred"),
        };

        let body = Json(serde_json::json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Failed to download file from S3: {0}")]
    Download(#[from] SdkError<GetObjectError>),

    #[error("Failed to upload file to s3: {0}")]
    Upload(#[from] SdkError<PutObjectError>),

    #[error("The requested file '{0}' was not found in the bucket")]
    FileNotFound(String),

    #[error("Local I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub enum DownloadError {
    FetchingS3MetadataError,
    FetchingMetadataError,
    MetadataNotFound,
    NotModified,
}

impl IntoResponse for DownloadError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            DownloadError::FetchingMetadataError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Error Fetching Metadata")
            }
            DownloadError::MetadataNotFound => (StatusCode::NOT_FOUND, "Metadata not found"),
            DownloadError::NotModified => (StatusCode::NOT_MODIFIED, "Not modified"),
            DownloadError::FetchingS3MetadataError => {
                (StatusCode::NOT_FOUND, "Metadata not found in s3")
            }
        };

        let body = Json(serde_json::json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

pub enum AppError {
    DatabaseTimeout,
    S3Timeout,
    SaasError,
}
