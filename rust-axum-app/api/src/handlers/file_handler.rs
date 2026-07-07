use aws_sdk_s3::Client;

use opentelemetry::KeyValue;
use sha2::Digest;
use sha2::Sha256;
use shared::s3_client::fetch_object_metadata;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::io::ReaderStream;
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

use crate::{dtos::file_dto::UploadSuccessResponseDto, types::app_state::AppState};
use crate::{
    dtos::file_dto::{DownloadRequestDto, UploadRequestDto},
    telemetry::metrics::ARTIFACTS_CREATED,
};
use crate::{telemetry::metrics::CACHE_LOOKUPS, types::error::DownloadError};
use shared::s3_client::{download_proxy_handler, upload_stream};
use shared::{
    models::ArtifactMetadata,
    repositories::artifact_metadata_repository::ArtifactMetadataRepository,
};

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
            Err(_) => {
                tracing::info!("Error processing stream");
            }
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
) -> Response {
    let url = format!(
        "{}/{}",
        query_meta.bucket_name.clone(),
        query_meta.file_name.clone()
    );

    let metadata = fetch_object_metadata(
        &state.saas_storage,
        query_meta.bucket_name.clone().to_string(),
        query_meta.file_name.clone().to_string(),
    )
    .await;

    if !state.should_passthrough {
        if let Ok(s3_metadata) = metadata {
            let result = ArtifactMetadataRepository::find_by_url(
                &&state.proxy_state.as_ref().unwrap().db,
                &url,
            )
            .await;

            let cache_client = &state.proxy_state.as_ref().unwrap().storage;

            match result {
                Ok(db_result) => match db_result {
                    Some(metadata) => {
                        let _metadata = metadata.clone();
                        //Evaluate etag field
                        let saas_etag = s3_metadata.etag.trim_matches('"').to_string(); //Remove RFC7232 specifications of sending back string etags with \"\"
                        let saas_last_modified = s3_metadata.last_modified.secs();

                        let proxy_etag = _metadata.etag.unwrap_or_else(|| {
                            tracing::info!("Error fetching etag");
                            return String::from("");
                        });

                        let proxy_last_modified = _metadata.last_modified.unwrap().timestamp();

                        tracing::info!(
                            "Proxy - Last Modified {proxy_last_modified} Etag : {proxy_etag}"
                        );
                        tracing::info!(
                            "SAAS - Last Modified {saas_last_modified} Etag : {saas_etag}"
                        );

                        //This shows that the proxy and saas have not gone out of sync
                        let is_match = (saas_etag == proxy_etag)
                            && (proxy_last_modified >= saas_last_modified);

                        if is_match {
                            tracing::info!("Cache found in proxy, pulling from cache");
                            return serve_from_cache(&cache_client, query_meta, metadata).await;
                        } else {
                            tracing::info!(
                                "Cache is out of date or empty. Pull from cache and refresh"
                            );
                            return serve_from_saas(state, query_meta).await;
                        }
                    }
                    None => {
                        tracing::info!("Did not find artifact metadata. Caching response");
                        return serve_from_saas(state, query_meta).await;
                    }
                },
                Err(_) => serve_from_saas(state, query_meta).await,
            }
        } else {
            serve_from_saas(state, query_meta).await
        }
    } else {
        serve_from_saas(state, query_meta).await
    }

    // tracing::info!("Logging");
}

async fn serve_from_saas(state: Arc<AppState>, query_meta: DownloadRequestDto) -> Response {
    CACHE_LOOKUPS.add(1, &[KeyValue::new("cache.hit", false)]);

    match download_proxy_handler(
        &state.saas_storage,
        &query_meta.bucket_name,
        &query_meta.file_name,
    )
    .await
    {
        Ok((mut stream, total_size)) => {
            let (client_tx, client_rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(32);

            let (cache_tx, mut cache_rx) = mpsc::channel::<Bytes>(32);
            let (checksum_tx, checksum_rx) = oneshot::channel::<String>();

            let query_key = query_meta.file_name.to_owned();
            let bucket_name = query_meta.bucket_name.to_owned();

            // let mut hasher = Sha256::new();
            // let mut collected: Vec<Bytes> = Vec::new();
            let state_one = state.clone();

            let url = format!(
                "{}/{}",
                query_meta.bucket_name.to_owned(),
                query_meta.file_name.to_owned()
            );

            tokio::spawn(
                async move {
                    match state_one.dashmap.entry(url.clone()) {
                        dashmap::Entry::Occupied(occupied_entry) => {
                            if occupied_entry.get().to_owned() {
                                tracing::info!("Value is already caching skip process");
                                return;
                            }
                        }
                        dashmap::Entry::Vacant(vacant_entry) => {
                            tracing::info!(
                                "Value is not cached, Add to dashmap and begin execution"
                            );
                            vacant_entry.insert(true);
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

                            let md5_val = upload_stream(
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
                                &md5_val,
                                &checksum,
                            )
                            .await
                            {
                                tracing::error!("Error adding file to proxy state {:?}", e)
                            }
                        }
                    }

                    state_one.dashmap.remove(&url).unwrap();
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
                    format!("attachment, filename=\"{}\"", query_meta.file_name),
                );

            builder
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(body)
                .unwrap()
        }
        Err(_) => {
            tracing::info!("Error fetching metadata from SAAS");
            return DownloadError::FetchingMetadataError.into_response();
        }
    }
}

pub async fn serve_from_cache(
    cache_client: &Client,
    query_meta: DownloadRequestDto,
    metadata: ArtifactMetadata,
) -> Response {
    CACHE_LOOKUPS.add(1, &[KeyValue::new("cache.hit", true)]);
    match download_proxy_handler(
        &cache_client,
        &query_meta.bucket_name,
        &query_meta.file_name,
    )
    .await
    {
        Ok((stream, total_size)) => {
            // let stream = stream.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));
            let async_read = stream.into_async_read();

            let stream = ReaderStream::new(async_read);
            let body = Body::from_stream(stream);

            let mut builder = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, total_size)
                .header("X-Processing-Mode", "NORMAL")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment, filename=\"{}\"", query_meta.file_name),
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
        Err(_) => DownloadError::FetchingS3MetadataError.into_response(),
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
    use shared::s3_client::{ensure_bucket, upload_stream};
    use tower::ServiceExt;

    use crate::{
        dtos::file_dto::UploadSuccessResponseDto, routes::create_router, types::app_state::AppState,
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
            .get_object()
            .bucket(bucket)
            .key(object)
            .send()
            .await
            .expect("Get object failed");

        let body = downloaded.body;

        let received: Bytes = body.collect().await.unwrap().into_bytes();

        assert_eq!(
            received, payload,
            "downloaded bytes must much the uploaded payload"
        );
    }

    async fn test_state() -> AppState {
        let _db = match shared::db::init_pool(&var("DATABASE_URL").unwrap().to_string()).await {
            Ok(client) => Some(client),
            Err(_) => {
                tracing::error!("Error initing DATABASE client");

                None
            }
        };

        let _storage = match shared::s3_client::init_s3_client().await {
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

        let _storage = match shared::s3_client::init_s3_client().await {
            Ok(client) => Some(client),
            Err(_) => {
                tracing::error!("Error initing minio client");

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
