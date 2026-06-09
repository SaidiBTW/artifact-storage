use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub enum AuthError {
    MissingToken,
    InvalidToken,
    TokenCreation,
    TokenExpired,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::MissingToken => (StatusCode::BAD_REQUEST, "Missing authorization"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::TokenCreation => (StatusCode::BAD_REQUEST, "Error during token creation"),
            AuthError::TokenExpired => (StatusCode::UNAUTHORIZED, "Token Expired"),
        };

        let body = Json(serde_json::json!({
      "error": error_message    }));

        (status, body).into_response()
    }
}
