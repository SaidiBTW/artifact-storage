use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct AuthResDto {
    pub access_token: String,
    pub token_type: String,
}

impl AuthResDto {
    pub fn new(access_token: String) -> Self {
        Self {
            access_token,
            token_type: "Bearer".to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginReqDto {
    pub email: String,
    pub password: String,
}
