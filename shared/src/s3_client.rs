use bytes::Bytes;
use futures_util::{Stream, StreamExt};

use minio::s3::response_traits::HasS3Fields;
use minio::s3::types::{Part, PartInfo, S3Api};
use minio::s3::{
    MinioClient, creds::StaticProvider, http::BaseUrl, response_traits::HasEtagFromHeaders,
    segmented_bytes::SegmentedBytes,
};
use sha2::{Digest, Sha256};
use std::env::var;
use std::fs::File;
use std::io::Write;
use std::io::{self, Error};
use std::pin::Pin;
pub async fn init_s3_client() -> MinioClient {
    dotenvy::dotenv().ok();
    let base_url = var("MINIO_URL")
        .expect("Minio URL should be defined")
        .parse::<BaseUrl>()
        .unwrap();
    let static_provider = StaticProvider::new("minioadmin", "minioadminpassword", None);
    let client = MinioClient::new(base_url, Some(static_provider), None, None).unwrap();

    client
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

// pub async fn download_stream(
//     client: &MinioClient,
//     bucket_name: &str,
//     object_name: &str,
// ) -> Result<Response, StatusCode> {
//     let response = client
//         .get_object(bucket_name, object_name)
//         .unwrap()
//         .build()
//         .send()
//         .await
//         .unwrap()
//         .request();

//     let stream = ReaderStream::new(response.)
// }

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
    let mut completed_parts = Vec::new();
    let mut hasher = Sha256::new();

    for (index, chunk) in chunks.into_iter().enumerate() {
        let part_number = (index + 1) as u16;
        let chunk_size = chunk.len();

        hasher.update(&chunk);
        let data = SegmentedBytes::from(chunk);

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
    }

    client
        .complete_multipart_upload(bucket_name, object_name, upload_id, completed_parts)
        .unwrap()
        .build()
        .send()
        .await
        .unwrap();

    let checksum = format!("{:x}", hasher.finalize());

    tracing::info!("Uploaded {} - SHA256: {}", object_name, checksum);
    Ok(checksum)
}

pub async fn download_proxy_handler(
    client: &MinioClient,
    bucket_name: &str,
    object_name: &str,
) -> io::Result<(
    Pin<Box<dyn Stream<Item = Result<Bytes, Error>> + Send>>,
    u64,
)> {
    let get_res = client
        .get_object(bucket_name, object_name)
        .unwrap()
        .build()
        .send()
        .await
        .unwrap();

    // let mut downloaded_bytes = 0;
    let (stream, size) = get_res.into_boxed_stream().unwrap();

    // let mapped_stream = byte_stream.map(|result| {
    //     result.map_err(|minio_err| {
    //         std::io::Error::new(std::io::ErrorKind::Other, minio_err.to_string())
    //     })
    // });

    // tracing::info!("Starting download of the file");

    // while let Some(chunk_result) = byte_stream.next().await {
    //     let chunk = chunk_result?;

    //     file.write_all(&chunk)?;

    //     downloaded_bytes += chunk.len() as u64;

    //     tracing::info!("Progress: {}/{} bytes", downloaded_bytes, total_size);
    // }

    // tracing::info!("Download Completed");

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
