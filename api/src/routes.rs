use axum::debug_handler;
use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    handlers::{
        auth_handler::login,
        file_handler::{download_handler, upload_handler},
        user_handler::get_user,
    },
    types::app_state::AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    let auth_routes = Router::new().route("/login", post(login));
    let user_routes = Router::new().route("/get", get(get_user));
    let file_routes = Router::new()
        .route("/upload", post(upload_handler))
        .route("/download", get(download_handler));

    Router::new()
        .route("/", get(|| async { "Auth Service Running" }))
        .nest("/auth", auth_routes)
        .nest("/user", user_routes)
        .nest("/file", file_routes)
        .with_state(app_state)
}

#[cfg(test)]
mod tests {
    use std::{env::var, fmt::format, sync::Arc};

    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use jsonwebtoken::{EncodingKey, Header, encode};
    use shared::s3_client::AppError;
    use tower::ServiceExt;
    use tower_http::classify::GrpcCode::Ok;

    use crate::{
        routes::create_router,
        services::auth_service::AuthService,
        types::{app_state::AppState, claims::Claims, keys::KEYS},
    };

    async fn test_state() -> AppState {
        let mut is_passthrough_state = false;

        let storage = shared::s3_client::init_s3_client().await.unwrap();
        let db = match shared::db::init_pool(
            &var("DATABASE_URL")
                .expect("DATABASE_URL not set")
                .to_string(),
        )
        .await
        {
            Result::Err(err) => {
                println!("{:?}", err);
                is_passthrough_state = true;
                Result::Err(AppError::DatabaseTimeout)
            }
            Result::Ok(pool) => Result::Ok(pool),
        };
        AppState::init().await.unwrap()
    }

    #[tokio::test]
    async fn test_missing_jwt_is_rejected() {
        dotenvy::dotenv().ok();
        let app = create_router(Arc::new(test_state().await));

        let req = Request::builder()
            .uri("/user/get")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_valid_jwt_is_accepted() {
        dotenvy::dotenv().ok();
        let auth_service = AuthService::new();
        let tokens = auth_service.generate_tokens("123456789").ok().unwrap();
        let app = create_router(Arc::new(test_state().await));

        let req = Request::builder()
            .uri("/user/get")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", tokens.access_token),
            )
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK)
    }
}
