use std::sync::LazyLock;

use axum::{body::Body, response::Response};
use dashmap::DashMap;
use dotenvy::var;
use minio::s3::MinioClient;
use moka::future::Cache;
use shared::{db::PgPool, s3_client::AppError};

use crate::services::auth_service::AuthService;

pub struct AppState {
    pub auth_service: AuthService,
    pub proxy_state: Option<ProxyState>,
    pub should_passthrough: bool,
    pub saas_storage: MinioClient,
    pub dashmap: DashMap<String, bool>, // This is a lock to act as a 'Exactly once semantic for caching operations
}

pub struct Config {
    pub database_url: String,
    pub minio_url: String,
    pub minio_saas_url: String,
    pub otel_service_name: String,
    pub otel_exporter_endpoint: String,
}

impl Config {
    pub fn new() -> Self {
        dotenvy::dotenv().ok();
        return Config {
            database_url: var("DATABASE_URL").unwrap().to_string().to_owned(),
            minio_url: var("MINIO_URL").unwrap().to_string().to_owned(),
            minio_saas_url: var("MINIO_SAAS_URL").unwrap().to_string().to_owned(),
            otel_service_name: var("OTEL_SERVICE_NAME").unwrap().to_string().to_owned(),
            otel_exporter_endpoint: var("OTEL_EXPORT_OTLP_ENDPOINT")
                .unwrap()
                .to_string()
                .to_owned(),
        };
    }
}
pub static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::new());

impl AppState {
    pub async fn init() -> Result<Self, AppError> {
        let mut is_passthrough_state = false;

        let saas_storage = shared::s3_client::init_saas_s3_client().await?;

        let storage = shared::s3_client::init_s3_client().await;
        let db = shared::db::init_pool(&CONFIG.database_url).await;

        if let Ok(storage) = storage {
            if let Ok(db) = db {
                return Ok(AppState {
                    auth_service: AuthService::new(),
                    proxy_state: Some(ProxyState {
                        db: db,
                        storage: storage,
                    }),
                    should_passthrough: is_passthrough_state,
                    saas_storage: saas_storage,
                    dashmap: DashMap::new(),
                });
            } else {
                is_passthrough_state = true;
            }
        } else {
            is_passthrough_state = true;
        };

        Ok(AppState {
            auth_service: AuthService::new(),
            proxy_state: None,
            should_passthrough: is_passthrough_state,
            saas_storage: saas_storage,
            dashmap: DashMap::new(),
        })
    }
}

pub struct ProxyState {
    pub storage: MinioClient,
    pub db: PgPool,
}
