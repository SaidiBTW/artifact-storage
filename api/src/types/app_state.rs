use std::sync::Arc;

use minio::s3::MinioClient;
use sqlx::postgres::PgPool;

use crate::services::auth_service::AuthService;

pub struct AppState {
    pub auth_service: AuthService,
    pub db: PgPool,
    pub storage: MinioClient,
}
