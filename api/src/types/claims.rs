use axum::RequestPartsExt;
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use jsonwebtoken::{Validation, decode};
use serde::{Deserialize, Serialize};

use crate::types::{error::AuthError, keys::KEYS};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

// #[derive(Clone)]
// pub struct AppState {
//     pub decoding_key: DecodingKey,
// }

// impl FromRef<AppState> for DecodingKey {
//     fn from_ref(input: &AppState) -> Self {
//         input.decoding_key.clone()
//     }
// }

impl<S> axum::extract::FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        if bearer.token().is_empty() {
            return Err(AuthError::MissingToken);
        }

        match decode::<Claims>(bearer.token(), &KEYS.decoding, &Validation::default()) {
            Ok(value) => {
                println!("Req from {} has just arrived", value.claims.sub);
                Ok(value.claims)
            }
            Err(_) => Err(AuthError::InvalidToken),
        }
    }
}
