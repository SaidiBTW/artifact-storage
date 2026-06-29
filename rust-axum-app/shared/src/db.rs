use sqlx::{Postgres, postgres::PgPoolOptions};
use sqlx_otel::PoolBuilder;

use std::time::Duration;

use crate::s3_client::AppError;

pub type PgPool = sqlx_otel::Pool<Postgres>;

pub async fn init_pool(database_url: &str) -> Result<PgPool, AppError> {
    let pool = PgPoolOptions::new()
        .max_connections(5) // Adjust based on your server capacity
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await;

    match pool {
        Ok(pool) => {
            let pool = PoolBuilder::from(pool)
                .with_database("mydatabase")
                .with_pool_name("my-metadata-db")
                .with_query_text_mode(sqlx_otel::QueryTextMode::Obfuscated)
                .with_pool_metrics_interval(Duration::from_secs(5))
                .build();

            Ok(pool)
        }
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
