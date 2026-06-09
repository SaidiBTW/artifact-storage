use axum::debug_handler;
use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

use axum_extra::extract::Query;

use bytes::Bytes;
use futures::StreamExt;

use shared::repositories::artifact_metadata_repository::ArtifactMetadataRepository;
use shared::{
    models::ArtifactMetadata,
    s3_client::{download_proxy_handler, upload_stream},
};

use crate::dtos::file_dto::{DownloadRequestDto, UploadRequestDto};
use crate::{dtos::file_dto::UploadSuccessResponseDto, types::app_state::AppState};

// async fn download_handler(State(state): State<Arc<AppState>>, Json(payload): Json<DownloadFileDto>) -> {}

#[debug_handler]
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
        &state.storage,
        &metadata.bucket_name,
        &metadata.file_name,
        chunks,
    )
    .await
    {
        Ok(checksum) => {
            let success_dto = UploadSuccessResponseDto {
                object_name: metadata.file_name.clone(),
                bucket: metadata.bucket_name.clone(),
                message: "File successfully uploaded".to_string(),
                size_bytes: total_upload_size,
            };
            tracing::info!("Saving file to db");
            ArtifactMetadataRepository::upsert(
                &state.db,
                &format!("{}/{}", &success_dto.bucket, success_dto.object_name),
                &checksum,
                &checksum,
            )
            .await
            .unwrap();

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
    Path(object_name): Path<String>,
    Query(metadata): Query<DownloadRequestDto>,
) -> Response {
    match download_proxy_handler(&state.storage, &metadata.bucket_name, &metadata.key).await {
        Ok((stream, total_size)) => {
            let mapped_stream = stream.map(|result| {
                result.map_err(|minio_err| {
                    std::io::Error::new(std::io::ErrorKind::Other, minio_err.to_string())
                })
            });

            let body = Body::from_stream(mapped_stream);

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, total_size)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment, filename=\"{}\"", object_name),
                )
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
        let client = shared::s3_client::init_s3_client().await;
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
        AppState {
            auth_service: AuthService::new(),
            db: shared::db::init_pool(
                &var("DATABASE_URL")
                    .expect("DATABASE_URL has not been set")
                    .to_string(),
            )
            .await,
            storage: shared::s3_client::init_s3_client().await,
        }
    }

    #[tokio::test]
    async fn test_upload_handler_returns_201() {
        dotenvy::dotenv().ok();
        let state = Arc::new(test_state().await);

        ensure_bucket(&state.storage, "test-bucket").await;

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
