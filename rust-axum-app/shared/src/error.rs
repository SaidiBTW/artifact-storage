use aws_sdk_s3::{
    error::{ProvideErrorMetadata, SdkError},
    operation::{
        complete_multipart_upload::CompleteMultipartUploadError,
        create_multipart_upload::CreateMultipartUploadError, get_object::GetObjectError,
        head_bucket::HeadBucketError, head_object::HeadObjectError, upload_part::UploadPartError,
    },
};
use axum::{Json, http::StatusCode, response::IntoResponse};
use thiserror::Error;
fn parse_s3_error<E, R>(error: SdkError<E, R>) -> StorageError
where
    E: ProvideErrorMetadata + std::fmt::Debug,
    R: std::fmt::Debug,
{
    match error {
        SdkError::ServiceError(err) => {
            let inner = err.err();
            let code = inner.code();

            if code == Some("NotFound") || code == Some("NoSuchKey") {
                return StorageError::FileNotFound;
            }
            if code == Some("AccessDenied") {
                return StorageError::AccessDenied;
            }

            StorageError::UnhandledS3Error(format!("Service Error: {:?}", inner))
        }

        SdkError::DispatchFailure(_) | SdkError::TimeoutError(_) => StorageError::NetworkIssue,
        _ => StorageError::UnhandledS3Error(format!("Unexpected SDK error: {:?}", error)),
    }
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("The request file could not be found")]
    FileNotFound,
    #[error("You do not have permission to access this file")]
    AccessDenied,
    #[error("Network or connection timeout")]
    NetworkIssue,
    #[error("An unexpected AWS error occurred: {0}")]
    UnhandledS3Error(String),
}

impl From<SdkError<GetObjectError>> for StorageError {
    fn from(error: SdkError<GetObjectError>) -> Self {
        parse_s3_error(error)
    }
}

impl From<SdkError<HeadBucketError>> for StorageError {
    fn from(error: SdkError<HeadBucketError>) -> Self {
        parse_s3_error(error)
    }
}
impl From<SdkError<HeadObjectError>> for StorageError {
    fn from(error: SdkError<HeadObjectError>) -> Self {
        parse_s3_error(error)
    }
}
impl From<SdkError<CreateMultipartUploadError>> for StorageError {
    fn from(error: SdkError<CreateMultipartUploadError>) -> Self {
        // Reuse the exact same logic we wrote earlier!
        parse_s3_error(error)
    }
}
impl From<SdkError<UploadPartError>> for StorageError {
    fn from(error: SdkError<UploadPartError>) -> Self {
        // Reuse the exact same logic we wrote earlier!
        parse_s3_error(error)
    }
}
impl From<SdkError<CompleteMultipartUploadError>> for StorageError {
    fn from(error: SdkError<CompleteMultipartUploadError>) -> Self {
        // Reuse the exact same logic we wrote earlier!
        parse_s3_error(error)
    }
}

impl IntoResponse for StorageError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            StorageError::AccessDenied => (StatusCode::UNAUTHORIZED, "Access Denied".to_string()),
            StorageError::FileNotFound => (StatusCode::NOT_FOUND, "Artifact Not Found".to_string()),
            StorageError::NetworkIssue => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Network Issue Reaching S3".to_string(),
            ),
            StorageError::UnhandledS3Error(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
        };

        let body = Json(serde_json::json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}
