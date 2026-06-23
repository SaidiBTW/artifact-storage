use sqlx::PgPool;

use crate::models::ArtifactMetadata;

pub struct ArtifactMetadataRepository;

impl ArtifactMetadataRepository {
    pub async fn find_by_url(
        pool: &PgPool,
        url: &str,
    ) -> Result<Option<ArtifactMetadata>, sqlx::Error> {
        sqlx::query_file_as!(
            ArtifactMetadata,
            "queries/artifact_metadata/find_artifact_metadata_by_url.sql",
            url
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn upsert(
        pool: &PgPool,
        url: &str,
        etag: &str,
        checksum: &str,
    ) -> Result<ArtifactMetadata, sqlx::Error> {
        tracing::info!("Upsert values");
        sqlx::query_file_as!(
            ArtifactMetadata,
            "queries/artifact_metadata/upsert.sql",
            url,
            checksum,
            etag
        )
        .fetch_one(pool)
        .await
    }
}
