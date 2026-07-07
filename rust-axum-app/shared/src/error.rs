use aws_sdk_s3::{
    error::SdkError,
    operation::{
        complete_multipart_upload::CompleteMultipartUploadError,
        create_multipart_upload::CreateMultipartUploadError, get_object::GetObjectError,
        put_object::PutObjectError, upload_part::UploadPartError,
    },
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Failed to start multipart upload: {0}")]
    CreateMultipartUpload(#[from] SdkError<CreateMultipartUploadError>),

    #[error("Failed to upload part: {0}")]
    UploadPartError(#[from] SdkError<UploadPartError>),

    #[error("Failed to download file from S3: {0}")]
    Download(#[from] SdkError<GetObjectError>),

    #[error("Failed to complete multi part upload")]
    CompleteMultipartUpload(#[from] SdkError<CompleteMultipartUploadError>),

    #[error("Failed to upload file to s3: {0}")]
    Upload(#[from] SdkError<PutObjectError>),

    #[error("The requested file '{0}' was not found in the bucket")]
    FileNotFound(String),

    #[error("Local I/O error: {0}")]
    Io(#[from] std::io::Error),
}
