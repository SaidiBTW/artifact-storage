use axum::Json;

use crate::{
    dtos::user_dto::GetUserDto,
    types::{claims::Claims, error::AuthError},
};

pub async fn get_user(claims: Claims) -> Result<Json<GetUserDto>, AuthError> {
    Ok(Json(GetUserDto {
        id: claims.sub,
        email: "iansawalasaidi@gmail.com".to_owned(),
        username: "Ian Sawala".to_owned(),
        info: "Hello".to_owned(),
    }))
}
