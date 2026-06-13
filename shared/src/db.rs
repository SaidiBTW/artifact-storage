use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

use crate::s3_client::AppError;

pub async fn init_pool(database_url: &str) -> Result<PgPool, AppError> {
    let pool = PgPoolOptions::new()
        .max_connections(5) // Adjust based on your server capacity
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await;

    match pool {
        Ok(pool) => Ok(pool),
        Err(error) => {
            println!("{:?}", error);
            tracing::info!("Error reaching postgres");
            Err(AppError::DatabaseTimeout)
        }
    }

    // Automatically run migrations on startup
    // sqlx::migrate!()
    //     .run(&pool)
    //     .await
    //     .expect("Failed to run database migrations");

    // Ok(pool)
}
