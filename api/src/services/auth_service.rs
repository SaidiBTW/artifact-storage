use jsonwebtoken::{Header, encode};

use crate::{
    dtos::auth_dto::AuthResDto,
    types::{claims::Claims, error::AuthError, keys::KEYS},
    utils::datetime::now_epoch,
};

const ACCESS_EXP_MINUTES: u32 = 15 * 60;

pub struct AuthService;

impl AuthService {
    pub fn new() -> Self {
        Self {}
    }

    pub fn generate_tokens(&self, user_id: &str) -> Result<AuthResDto, AuthError> {
        let claims = Claims {
            sub: user_id.to_owned(),
            exp: now_epoch() + ACCESS_EXP_MINUTES as usize,
        };

        let access_token = encode(&Header::default(), &claims, &KEYS.encoding)
            .map_err(|_| AuthError::TokenCreation)?;

        Ok(AuthResDto::new(access_token))
    }
}
