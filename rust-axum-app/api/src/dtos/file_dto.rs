use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadSuccessResponseDto {
    pub object_name: String,
    pub bucket: String,
    pub size_bytes: usize,
    pub message: String,
    pub is_passthrough: bool,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct UploadRequestDto {
    pub file_name: String,
    pub bucket_name: String,
}

impl Default for UploadRequestDto {
    fn default() -> Self {
        Self {
            file_name: Default::default(),
            bucket_name: Default::default(),
        }
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct DownloadRequestDto {
    pub file_name: String,
    pub bucket_name: String,
}
