use bytes::{Bytes, BytesMut};
use futures_util::Stream;

use minio::s3::MinioClientBuilder;
use minio::s3::client::DEFAULT_REGION;
use minio::s3::types::{ETag, PartInfo, Region, S3Api};
use minio::s3::utils::UtcTime;
use minio::s3::{
    MinioClient, creds::StaticProvider, http::BaseUrl, response_traits::HasEtagFromHeaders,
    segmented_bytes::SegmentedBytes,
};
use sha2::{Digest, Sha256};
use sqlx::types::chrono::Utc;
use std::env::var;
use std::fs::File;
use std::io::Write;
use std::io::{self, Error};
use std::pin::Pin;
use std::sync::{LazyLock, mpsc};

const UPLOAD_MULTI_PART_SIZE: usize = 64 * 1024 * 1024; //64 MB
pub async fn init_s3_client() -> Result<MinioClient, AppError> {
    tracing::info!("Intiing S3 Client");
    dotenvy::dotenv().ok();
    let base_url = var("GARAGE_URL")
        .expect("Minio URL should be defined")
        .parse::<BaseUrl>()
        .unwrap();
    let access_key = var("GARAGE_DEFAULT_ACCESS_KEY").expect("Access key required");
    let secret = var("GARAGE_DEFAULT_SECRET_KEY").expect("Secret Key Required");

    let static_provider = StaticProvider::new(&access_key, &secret, None);
    let client = MinioClientBuilder::new(base_url)
        .provider(Some(static_provider))
        .ignore_cert_check(None)
        .skip_region_lookup(true)
        .build()
        .unwrap();

    match client.list_buckets().build().send().await {
        Ok(_) => {
            tracing::info!("Can list buckets. Minio Is online")
        }
        Err(err) => match err {
            minio::s3::error::Error::Network(network_error) => match network_error {
                minio::s3::error::NetworkError::ServerError(_) => {
                    tracing::error!("Server Error experienced due to wrong URL");
                    return Err(AppError::MinioTimeout);
                }
                minio::s3::error::NetworkError::ReqwestError(error) => {
                    tracing::error!("Server Error experienced due to wrong URL");
                    return Err(AppError::MinioTimeout);
                }
            },
            // err => {
            //     tracing::info!("Error {:?}", err)
            // }
            minio::s3::error::Error::S3Server(s3_server_error) => match s3_server_error {
                minio::s3::error::S3ServerError::InvalidServerResponse {
                    message,
                    http_status_code,
                    content_type,
                } => {
                    tracing::info!("Cannot connect to server");
                    return Err(AppError::MinioTimeout);
                }
                _ => {
                    tracing::info!("Unknown Error : {:?}", s3_server_error);
                }
            },
            _ => {
                tracing::info!("Unhandled Error: {:?}", err)
            } // minio::s3::error::Error::DriveIo(io_error) => todo!(),
              // minio::s3::error::Error::Validation(validation_err) => todo!(),
        },
    };

    Ok(client)
}

pub async fn init_saas_s3_client() -> Result<MinioClient, AppError> {
    tracing::info!("Initializaing SAAS Client");
    dotenvy::dotenv().ok();

    let base_url = var("GARAGE_SAAS_URL")
        .expect("Minio SAAS Url not defined")
        .parse::<BaseUrl>()
        .unwrap();
    let access_key = var("GARAGE_DEFAULT_ACCESS_KEY").expect("Access key required");
    let secret = var("GARAGE_DEFAULT_SECRET_KEY").expect("Secret Key Required");

    let static_provider = StaticProvider::new(&access_key, &secret, None);
    // DEFAULT_REGION = LazyLock::new(|| Region::new("garage").unwrap());

    let client = MinioClientBuilder::new(base_url)
        .provider(Some(static_provider))
        .ignore_cert_check(None)
        .skip_region_lookup(true)
        .build()
        .unwrap();

    match client.list_buckets().build().send().await {
        Ok(_) => {
            tracing::info!("Can list bucket. SAAS Minio is online");
        }
        Err(err) => match err {
            minio::s3::error::Error::Network(network_error) => match network_error {
                minio::s3::error::NetworkError::ServerError(_) => {
                    tracing::error!("Server Error experienced due to wrong URL");
                    return Err(AppError::SaasS3Error);
                }
                minio::s3::error::NetworkError::ReqwestError(error) => {
                    tracing::error!("Server Error experienced due to wrong URL");
                    return Err(AppError::SaasS3Error);
                }
            },
            // err => {
            //     tracing::info!("Error {:?}", err)
            // }
            minio::s3::error::Error::S3Server(s3_server_error) => match s3_server_error {
                minio::s3::error::S3ServerError::InvalidServerResponse {
                    message,
                    http_status_code,
                    content_type,
                } => {
                    tracing::info!("Cannot connect to server");
                    return Err(AppError::SaasS3Error);
                }
                _ => {}
            },
            _ => todo!(),
        },
    };
    Ok(client)
}
pub async fn verify_and_setup_storage(client: &minio::s3::client::MinioClient) {
    tracing::info!("Fetching buckets");

    let response = client
        .list_buckets()
        .build()
        .send()
        .await
        .expect("Failed to list buckets");

    for bucket in response.buckets().unwrap() {
        tracing::info!("Found bucket: {}", bucket.name);
    }

    let bucket_name = "artifacts";

    let exists = client
        .bucket_exists(bucket_name)
        .unwrap()
        .build()
        .send()
        .await
        .expect("request failed");

    if exists.exists() {
        tracing::info!("Bucket '{}' already exists", bucket_name);
    } else {
        tracing::info!("Creating bucket '{}' ...", bucket_name);
        client
            .create_bucket(bucket_name)
            .unwrap()
            .build()
            .send()
            .await
            .unwrap();

        tracing::info!("Bucket created successfully");
    }
}
pub async fn upload_stream(
    client: &MinioClient,
    bucket_name: &str,
    object_name: &str,
    chunks: Vec<Bytes>,
) -> io::Result<String> {
    let upload = client
        .create_multipart_upload(bucket_name, object_name)
        .unwrap()
        .build()
        .send()
        .await
        .unwrap();
    let upload_id = upload.upload_id().await.unwrap();
    let mut combined_hashes = Vec::new();
    let mut completed_parts = Vec::new();
    let mut buffer = BytesMut::new();
    let mut part_number = 1;

    for chunk in chunks {
        buffer.extend_from_slice(&chunk);

        while buffer.len() >= UPLOAD_MULTI_PART_SIZE {
            let part_data = buffer.split_to(UPLOAD_MULTI_PART_SIZE).freeze();
            let chunk_size = part_data.len();

            let digest = md5::compute(&part_data);
            let data = SegmentedBytes::from(part_data);

            let part_res = client
                .upload_part(bucket_name, object_name, &upload_id, part_number, data)
                .unwrap()
                .build()
                .send()
                .await
                .unwrap();

            completed_parts.push(PartInfo {
                etag: part_res.etag().unwrap(),
                number: part_number,
                size: chunk_size as u64,
                checksum: None,
            });

            combined_hashes.extend_from_slice(&digest.0);

            part_number += 1;
        }
    }

    if !buffer.is_empty() || part_number == 1 {
        let part_data = buffer.freeze();
        let chunk_size = part_data.len();

        let digest = md5::compute(&part_data);
        let data = SegmentedBytes::from(part_data);
        let part_res = client
            .upload_part(bucket_name, object_name, &upload_id, part_number, data)
            .unwrap()
            .build()
            .send()
            .await
            .unwrap();

        combined_hashes.extend_from_slice(&digest.0);

        completed_parts.push(PartInfo {
            etag: part_res.etag().unwrap(),
            number: part_number,
            size: chunk_size as u64,
            checksum: None,
        });
    }

    client
        .complete_multipart_upload(bucket_name, object_name, upload_id, completed_parts)
        .unwrap()
        .build()
        .send()
        .await
        .unwrap();

    let final_digest = md5::compute(combined_hashes);

    let value = format!("{:x}-{}", final_digest, part_number);

    tracing::info!("Uploaded {} - MD5: {}", object_name, value);
    Ok(value)
}

pub async fn download_proxy_handler(
    client: &MinioClient,
    bucket_name: &str,
    object_name: &str,
) -> io::Result<(
    Pin<Box<dyn Stream<Item = Result<Bytes, Error>> + Send>>,
    u64,
)> {
    tracing::info!("{bucket_name}/{object_name} => Fetching object");
    let get_res = client
        .get_object(bucket_name, object_name)
        .unwrap()
        .build()
        .send()
        .await
        .unwrap();

    let (stream, size) = get_res.into_boxed_stream().unwrap();

    Ok((stream, size))
}

pub async fn ensure_bucket(client: &MinioClient, bucket_name: &str) {
    let exists = client
        .bucket_exists(bucket_name)
        .unwrap()
        .build()
        .send()
        .await
        .unwrap();
    if !exists.exists() {
        client
            .create_bucket(bucket_name)
            .unwrap()
            .build()
            .send()
            .await
            .unwrap();
    }
}

pub async fn fetch_object_metadata(
    client: &MinioClient,
    bucket_name: String,
    file_name: String,
) -> Result<S3Metadata, AppError> {
    let object = client
        .stat_object(bucket_name, file_name)
        .unwrap()
        .build()
        .send()
        .await;
    tracing::info!("Response metadata gotten back {:?}", object);

    match object {
        Ok(object) => {
            return Ok(S3Metadata {
                size: object.size().unwrap(),
                etag: object.etag().unwrap().to_string(),
                last_modified: object.last_modified().unwrap(),
            });
        }
        Err(e) => Err(AppError::S3MetadataNotFound),
    }
}

#[derive(Debug, Clone)]
pub struct S3Metadata {
    pub size: u64,
    pub etag: String,
    pub last_modified: Option<UtcTime>,
}

#[derive(Debug)]
pub enum AppError {
    DatabaseTimeout,
    MinioTimeout,
    ProxyError,
    SaasS3Error,
    S3MetadataNotFound,
}
