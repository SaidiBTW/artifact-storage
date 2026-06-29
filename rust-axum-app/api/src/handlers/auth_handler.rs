use std::sync::Arc;

use axum::{Json, extract::State};

use crate::{
    dtos::auth_dto::{AuthResDto, LoginReqDto},
    types::{app_state::AppState, error::AuthError},
};

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginReqDto>,
) -> Result<Json<AuthResDto>, AuthError> {
    if payload.email.is_empty() || !payload.email.contains("@") {
        return Err(AuthError::MissingToken);
    }

    if payload.email == "iansawalasaidi@gmail.com" && payload.password == "IanSawala" {
        let tokens = state.auth_service.generate_tokens("123456789")?;
        return Ok(Json(tokens));
    }

    Err(AuthError::InvalidToken)
}
