use aws_config::Region;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::primitives::{ByteStream, DateTime};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::{Client, Config};
use bytes::{Bytes, BytesMut};

use std::env::var;

use crate::error::StorageError;

const UPLOAD_MULTI_PART_SIZE: usize = 64 * 1024 * 1024; //64 MB
pub async fn init_s3_client() -> Result<Client, AppError> {
    tracing::info!("Initing S3 Client");
    dotenvy::dotenv().ok();
    let access_key = var("GARAGE_DEFAULT_ACCESS_KEY").expect("Access key required");
    let secret = var("GARAGE_DEFAULT_SECRET_KEY").expect("Secret Key Required");
    let garage_url = var("GARAGE_URL").expect("GARAGE_URL required");

    let credentials = Credentials::new(access_key, secret, None, None, "static-provider");

    let config = Config::builder()
        .credentials_provider(credentials)
        .behavior_version_latest()
        .region(Region::new("garage"))
        .endpoint_url(garage_url)
        .force_path_style(true)
        .build();

    let client = Client::from_conf(config);

    match client.list_buckets().send().await {
        Ok(_) => {
            tracing::info!("Can list buckets. Minio Is online")
        }
        Err(err) => {
            tracing::info!("Error listing bucket {:?}", err)
        }
    };

    Ok(client)
}

pub async fn init_saas_s3_client() -> Result<Client, AppError> {
    tracing::info!("Initializing SAAS Client");
    dotenvy::dotenv().ok();
    let access_key = var("GARAGE_DEFAULT_ACCESS_KEY").expect("Access key required");
    let secret = var("GARAGE_DEFAULT_SECRET_KEY").expect("Secret Key Required");
    let garage_saas_url = var("GARAGE_SAAS_URL").expect("GARAGE_SAAS_URL required");

    let credentials = Credentials::new(access_key, secret, None, None, "static-provider");

    let config = Config::builder()
        .credentials_provider(credentials)
        .behavior_version_latest()
        .region(Region::new("garage"))
        .endpoint_url(garage_saas_url)
        .force_path_style(true)
        .build();

    let client = Client::from_conf(config);

    match client.list_buckets().send().await {
        Ok(_) => {
            tracing::info!("Can list bucket. SAAS S3 is online");
        }
        Err(err) => {
            tracing::info!("Error listing buckets {:?}", err)
        }
    };
    Ok(client)
}
pub async fn verify_and_setup_storage(client: &Client) {
    tracing::info!("Fetching buckets");

    let response = client
        .list_buckets()
        .send()
        .await
        .expect("Failed to list buckets");

    for bucket in response.buckets() {
        tracing::info!("Found bucket: {:?}", bucket.name);
    }

    let bucket_name = "artifacts";

    let bucket = client
        .head_bucket()
        .set_bucket(Some(bucket_name.to_string()))
        .send()
        .await;

    match bucket {
        Ok(_) => {
            tracing::info!("Bucket '{}' already exists in region", bucket_name)
        }
        Err(err) => {
            tracing::info!("Head Bucket err: {}", err);
        }
    }
}
pub async fn upload_stream(
    client: &Client,
    bucket_name: &str,
    object_name: &str,
    chunks: Vec<Bytes>,
) -> Result<String, StorageError> {
    let upload = client
        .create_multipart_upload()
        .bucket(bucket_name)
        .key(object_name)
        .send()
        .await?;
    let upload_id = upload.upload_id().unwrap();
    let mut combined_hashes = Vec::new();
    let mut completed_parts = Vec::new();
    let mut buffer = BytesMut::new();
    let mut part_number = 1;

    for chunk in chunks {
        buffer.extend_from_slice(&chunk);

        while buffer.len() >= UPLOAD_MULTI_PART_SIZE {
            let part_data = buffer.split_to(UPLOAD_MULTI_PART_SIZE).freeze();
            let _chunk_size = part_data.len();

            let digest = md5::compute(&part_data);

            let part_res = client
                .upload_part()
                .set_bucket(Some(bucket_name.to_string()))
                .set_key(Some(object_name.to_string()))
                .set_upload_id(Some(upload_id.to_string()))
                .set_part_number(Some(part_number))
                .body(ByteStream::from(part_data))
                .send()
                .await?;

            completed_parts.push(
                CompletedPart::builder()
                    .part_number(part_number)
                    .e_tag(part_res.e_tag.unwrap().to_string())
                    .build(),
            );

            combined_hashes.extend_from_slice(&digest.0);

            part_number += 1;
        }
    }

    if !buffer.is_empty() || part_number == 1 {
        let part_data = buffer.freeze();
        let _chunk_size = part_data.len();

        let digest = md5::compute(&part_data);
        let part_res = client
            .upload_part()
            .set_body(Some(ByteStream::from(part_data)))
            .set_part_number(Some(part_number))
            .set_upload_id(Some(upload_id.to_string()))
            .set_key(Some(object_name.to_string()))
            .set_bucket(Some(bucket_name.to_string()))
            .send()
            .await?;

        combined_hashes.extend_from_slice(&digest.0);

        completed_parts.push(
            CompletedPart::builder()
                .part_number(part_number)
                .e_tag(part_res.e_tag.unwrap().to_string())
                .build(),
        );
    }

    client
        .complete_multipart_upload()
        .set_bucket(Some(bucket_name.to_string()))
        .set_key(Some(object_name.to_string()))
        .set_upload_id(Some(upload_id.to_string()))
        .multipart_upload(
            CompletedMultipartUpload::builder()
                .set_parts(Some(completed_parts))
                .build(),
        )
        .send()
        .await?;

    let final_digest = md5::compute(combined_hashes);

    let value = format!("{:x}-{}", final_digest, part_number);

    tracing::info!("Uploaded {} - MD5: {}", object_name, value);
    Ok(value)
}

pub async fn download_proxy_handler(
    client: &Client,
    bucket_name: &str,
    object_name: &str,
) -> Result<(ByteStream, u64), StorageError> {
    tracing::info!("{bucket_name}/{object_name} => Fetching object");
    let get_res = client
        .get_object()
        .set_bucket(Some(bucket_name.to_string()))
        .set_key(Some(object_name.to_string()))
        .send()
        .await?;

    let (stream, size) = (get_res.body, get_res.content_length.unwrap());

    Ok((stream, size as u64))
}

pub async fn ensure_bucket(client: &Client, bucket_name: &str) {
    let exists = client
        .head_bucket()
        .set_bucket(Some(bucket_name.to_string()))
        .send()
        .await;

    match exists {
        Ok(_) => {}
        Err(error) => {
            tracing::info!("Error in ensure_bucket: {:?}", error)
        }
    }
}

pub async fn fetch_object_metadata(
    client: &Client,
    bucket_name: String,
    file_name: String,
) -> Result<S3Metadata, AppError> {
    let object = client
        .head_object()
        .set_bucket(Some(bucket_name))
        .set_key(Some(file_name))
        .send()
        .await;

    match object {
        Ok(object) => {
            return Ok(S3Metadata {
                size: object.content_length().unwrap() as u64,
                etag: object.e_tag().unwrap().to_string(),
                last_modified: object.last_modified.unwrap(),
            });
        }
        Err(e) => {
            tracing::info!("Error : {:?}", e);

            Err(AppError::S3MetadataNotFound)
        }
    }
}

#[derive(Debug, Clone)]
pub struct S3Metadata {
    pub size: u64,
    pub etag: String,
    pub last_modified: DateTime,
}

#[derive(Debug)]
pub enum AppError {
    DatabaseTimeout,
    MinioTimeout,
    ProxyError,
    SaasS3Error,
    S3MetadataNotFound,
}
