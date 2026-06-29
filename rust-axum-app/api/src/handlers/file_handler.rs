use axum::http::HeaderMap;
use minio::s3::MinioClient;
use opentelemetry::KeyValue;
use sha2::Digest;
use sha2::Sha256;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tracing::Instrument;

use axum::{
    Json,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use axum_extra::extract::Query;

use bytes::Bytes;
use futures::StreamExt;

use shared::s3_client::{download_proxy_handler, upload_stream};
use shared::{
    models::ArtifactMetadata,
    repositories::artifact_metadata_repository::ArtifactMetadataRepository,
};

use crate::{dtos::file_dto::UploadSuccessResponseDto, types::app_state::AppState};
use crate::{
    dtos::file_dto::{DownloadRequestDto, UploadRequestDto},
    telemetry::metrics::ARTIFACTS_CREATED,
};
use crate::{telemetry::metrics::CACHE_LOOKUPS, types::error::DownloadError};

// async fn download_handler(State(state): State<Arc<AppState>>, Json(payload): Json<DownloadFileDto>) -> {}

// #[debug_handler]
pub async fn upload_handler(
    // request: Body,
    State(state): State<Arc<AppState>>,
    Query(metadata): Query<UploadRequestDto>, // <-- 1. Axum parses the query string for you
    req: Request,
) -> Response {
    let mut chunks = Vec::new();
    let mut total_upload_size = 0;

    let body = req.into_body();
    // let metadata = op
    //
    let mut stream = body.into_data_stream();
    while let Some(chunk_res) = stream.next().await {
        match chunk_res {
            Ok(bytes) => {
                total_upload_size += bytes.len();
                chunks.push(bytes);
            }
            Err(e) => todo!(),
        }
    }

    match upload_stream(
        &state.saas_storage,
        &metadata.bucket_name,
        &metadata.file_name,
        chunks,
    )
    .await
    {
        Ok(_) => {
            let success_dto = UploadSuccessResponseDto {
                object_name: metadata.file_name.clone(),
                bucket: metadata.bucket_name.clone(),
                message: "File successfully uploaded".to_string(),
                size_bytes: total_upload_size,
                is_passthrough: state.should_passthrough.to_owned(),
            };

            ARTIFACTS_CREATED.add(1, &[]);

            // if !state.should_passthrough.to_owned() {
            //     // let state = Arc::clone(&state);
            //     // tracing::info!("Saving file to db");
            //     // let result = ArtifactMetadataRepository::upsert(
            //     //     &state.proxy_state.as_ref().unwrap().db,
            //     //     &format!("{}/{}", &success_dto.bucket, success_dto.object_name),
            //     //     &checksum,
            //     //     &checksum,
            //     // )
            //     // .await;

            //     match result {
            //         Ok(result) => {
            //             return (StatusCode::CREATED, Json(success_dto)).into_response();
            //         }
            //         Err(Error::Database(db_err)) => {
            //             let pg_err = db_err.downcast_ref::<PgDatabaseError>();
            //             if pg_err.code() == "23505" {
            //                 tracing::error!("Unique vioation for {}", checksum);
            //                 return UploadError::UploadConflict.into_response();
            //             } else {
            //                 return UploadError::OtherError.into_response();
            //             }
            //         }
            //         Err(e) => {
            //             return (
            //                 StatusCode::INTERNAL_SERVER_ERROR,
            //                 Json::from("Error Unique violation constraint"),
            //             )
            //                 .into_response();
            //         }
            //     }
            // }
            return (StatusCode::CREATED, Json(success_dto)).into_response();
        }
        Err(e) => {
            tracing::error!("Minio Upload failed {:?}", e);

            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

pub async fn download_handler(
    State(state): State<Arc<AppState>>,
    Query(query_meta): Query<DownloadRequestDto>,
    headers: HeaderMap,
) -> Response {
    let url = format!("{}/{}", query_meta.bucket_name, query_meta.key);
    // tracing::info!("Logging");

    if !state.should_passthrough {
        let result =
            ArtifactMetadataRepository::find_by_url(&&state.proxy_state.as_ref().unwrap().db, &url)
                .await;

        let cache_client = &state.proxy_state.as_ref().unwrap().storage;

        match result {
            Ok(db_result) => match db_result {
                Some(metadata) => {
                    //Evaluate IF-NONE_MATCH header
                    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
                        let client_etag = if_none_match.to_str().unwrap_or("");
                        if let Some(ref etag) = metadata.etag {
                            let is_match = client_etag == "*" || etag == client_etag;

                            if is_match {
                                tracing::error!("Not modified since last pull");
                                return DownloadError::NotModified.into_response();
                            }
                        }
                    }

                    //Evaluat IF_MODIFIED_SINCE header
                    if let Some(if_modified_since) = headers.get(header::IF_MODIFIED_SINCE) {
                        let last_modified = metadata.last_modified;

                        if let Ok(since_str) = if_modified_since.to_str() {
                            if let Ok(since) = DateTime::parse_from_rfc2822(since_str) {
                                let since: DateTime<Utc> = since.into();
                                if let Some(last_modified) = last_modified {
                                    let last_mod_secs = last_modified.timestamp();
                                    let since_secs = since.timestamp();
                                    println!("{:?} Since-{:?}", last_mod_secs, since_secs);

                                    if last_mod_secs <= since_secs {
                                        return DownloadError::NotModified.into_response();
                                    }
                                }
                            }
                        }
                    }
                    return serve_from_cache(&cache_client, query_meta, metadata).await;
                }
                None => {
                    tracing::info!("Did not find artifact metadata. Caching response");
                    return server_from_saas(state, query_meta).await;
                }
            },
            Err(_) => {
                tracing::info!("Error fetching metadata from DB");
                return DownloadError::FetchingMetadataError.into_response();
            }
        }
    } else {
        server_from_saas(state, query_meta).await
    }
}

async fn server_from_saas(state: Arc<AppState>, query_meta: DownloadRequestDto) -> Response {
    CACHE_LOOKUPS.add(1, &[KeyValue::new("cache.hit", true)]);

    match download_proxy_handler(
        &state.saas_storage,
        &query_meta.bucket_name,
        &query_meta.key,
    )
    .await
    {
        Ok((stream, total_size)) => {
            let mapped_stream = stream.map(|result| {
                result.map_err(|minio_err| {
                    std::io::Error::new(std::io::ErrorKind::Other, minio_err.to_string())
                })
            });

            let (client_tx, client_rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);

            let (cache_tx, mut cache_rx) = mpsc::channel::<Bytes>(32);
            let (checksum_tx, mut checksum_rx) = oneshot::channel::<String>();

            let query_key = query_meta.key.to_owned();
            let bucket_name = query_meta.bucket_name.to_owned();

            // let mut hasher = Sha256::new();
            // let mut collected: Vec<Bytes> = Vec::new();
            let mut stream = mapped_stream;
            let state_one = state.clone();

            tokio::spawn(
                async move {
                    let mut parts: Vec<Bytes> = Vec::new();

                    while let Some(chunk) = cache_rx.recv().await {
                        parts.push(chunk);
                    }

                    let checksum = match checksum_rx.await {
                        Ok(c) => {
                            tracing::info!("Checksum {c}");
                            c
                        }
                        Err(_) => {
                            tracing::info!("Failed to receive checksum");
                            return;
                        }
                    };

                    upload_stream(
                        &state_one.proxy_state.as_ref().unwrap().storage,
                        &bucket_name,
                        &query_key,
                        parts,
                    )
                    .await
                    .unwrap();

                    let url = format!("{}/{}", &bucket_name, &query_key);

                    if let Err(e) = ArtifactMetadataRepository::upsert(
                        &state_one.proxy_state.as_ref().unwrap().db,
                        &url,
                        &checksum,
                        &checksum,
                    )
                    .await
                    {
                        tracing::error!("Error adding file to proxy state")
                    }
                }
                .in_current_span(),
            );
            tokio::spawn(
                async move {
                    let mut hasher = Sha256::new();
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                hasher.update(&bytes);
                                let _ = cache_tx.send(bytes.clone()).await;

                                let _ = client_tx.send(Ok(bytes)).await;
                            }
                            Err(e) => {
                                let _ = client_tx
                                    .send(Err(std::io::Error::new(std::io::ErrorKind::Other, e)))
                                    .await;
                                return;
                            }
                        }
                    }
                    let hash_result = format!("{:x}", hasher.finalize());
                    let _ = checksum_tx.send(hash_result);
                }
                .in_current_span(),
            );

            let stream = ReceiverStream::new(client_rx);
            let body = Body::from_stream(stream);

            let builder = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, total_size)
                .header("X-Processing-Mode", "PASSTHROUGH")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment, filename=\"{}\"", query_meta.key),
                );

            builder
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(body)
                .unwrap()
        }
        Err(_) => todo!(),
    }
}

pub async fn serve_from_cache(
    cache_client: &MinioClient,
    query_meta: DownloadRequestDto,
    metadata: ArtifactMetadata,
) -> Response {
    CACHE_LOOKUPS.add(1, &[KeyValue::new("cache.hit", false)]);
    match download_proxy_handler(&cache_client, &query_meta.bucket_name, &query_meta.key).await {
        Ok((stream, total_size)) => {
            let mapped_stream = stream.map(|result| {
                result.map_err(|minio_err| {
                    std::io::Error::new(std::io::ErrorKind::Other, minio_err.to_string())
                })
            });

            let body = Body::from_stream(mapped_stream);

            let mut builder = Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(header::CONTENT_LENGTH, total_size)
                .header("X-Processing-Mode", "NORMAL")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment, filename=\"{}\"", query_meta.key),
                );

            if let Some(ref last_modified) = metadata.last_modified {
                builder = builder.header(
                    header::LAST_MODIFIED,
                    last_modified
                        .format("%a, %d %b %Y %H:%M:%S GMT")
                        .to_string(),
                );
            }

            if metadata.etag.is_some() {
                builder = builder.header(header::IF_NONE_MATCH, metadata.etag.unwrap());
            }

            builder
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(body)
                .unwrap()
        }
        Err(_) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use bytes::Bytes;
    use dotenvy::var;
    use minio::s3::types::S3Api;
    use shared::s3_client::{ensure_bucket, upload_stream};
    use tower::ServiceExt;

    use crate::{
        dtos::file_dto::UploadSuccessResponseDto, routes::create_router,
        services::auth_service::AuthService, types::app_state::AppState,
    };

    #[tokio::test]
    async fn test_upload_stream_single_chunk() {
        let client = shared::s3_client::init_s3_client().await.unwrap();
        let bucket = "test-bucket";
        let object = "single-chunk.bin";

        shared::s3_client::ensure_bucket(&client, bucket).await;

        let payload = Bytes::from_static(b"hello minio single chunk");
        let chunks = vec![payload.clone()];

        upload_stream(&client, bucket, object, chunks)
            .await
            .expect("upload stream");

        //Verify Object exists
        let downloaded = client
            .get_object(bucket, object)
            .unwrap()
            .build()
            .send()
            .await
            .expect("Get object failed");

        let body = downloaded
            .content()
            .unwrap()
            .to_segmented_bytes()
            .await
            .unwrap();

        let received: Bytes = body.to_bytes();

        assert_eq!(
            received, payload,
            "downloaded bytes must much the uploaded payload"
        );
    }

    async fn test_state() -> AppState {
        let mut is_passthrough_state = false;
        let db = match shared::db::init_pool(&var("DATABASE_URL").unwrap().to_string()).await {
            Ok(client) => Some(client),
            Err(_) => {
                tracing::error!("Error initing DATABASE client");
                is_passthrough_state = true;

                None
            }
        };

        let storage = match shared::s3_client::init_s3_client().await {
            Ok(client) => Some(client),
            Err(_) => {
                tracing::error!("Error initing minio client");
                None
            }
        };
        AppState::init().await.unwrap()
    }

    #[tokio::test]
    async fn test_upload_handler_returns_201() {
        dotenvy::dotenv().ok();
        let state = Arc::new(test_state().await);

        let mut is_passthrough_state = false;

        let storage = match shared::s3_client::init_s3_client().await {
            Ok(client) => Some(client),
            Err(_) => {
                tracing::error!("Error initing minio client");
                is_passthrough_state = true;

                None
            }
        };

        ensure_bucket(&state.saas_storage, "test-bucket").await;

        let payload = b"integration test payload";
        let app = create_router(state);

        let request = Request::builder()
            .method("POST")
            .uri("/file/upload?bucket_name=test-bucket&file_name=integration.txt")
            .header("content-type", "application/octet-stream")
            .body(Body::from(payload.as_ref()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        let dto: UploadSuccessResponseDto = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(dto.bucket, "test-bucket");
        assert_eq!(dto.object_name, "integration.txt");
        assert_eq!(dto.size_bytes, payload.len());
    }
}
