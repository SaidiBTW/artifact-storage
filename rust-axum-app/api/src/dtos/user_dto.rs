use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GetUserDto {
    pub id: String,
    pub email: String,
    pub username: String,
    pub info: String,
}
