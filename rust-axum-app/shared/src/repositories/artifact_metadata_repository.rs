use crate::{db::PgPool, models::ArtifactMetadata};
use chrono::{DateTime, Utc};
use tracing::instrument;

pub struct ArtifactMetadataRepository;

impl ArtifactMetadataRepository {
    #[instrument(name = "db.artifact_metadata.find_artifact_metadata_by_url",
        fields(
            db.system = "postgresql",
            db.operation = "SELECT",
            artifact.url = %url
        ))]
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
    #[instrument(name = "db.artifact_metadata.upsert", fields(
        db.system = "postgresql",
        db.operation = "INSERT",
        artifact.url = %url
    ))]
    pub async fn upsert(
        pool: &PgPool,
        url: &str,
        etag: &str,
        checksum: &str,
        last_modified: &DateTime<Utc>,
    ) -> Result<ArtifactMetadata, sqlx::Error> {
        sqlx::query_file_as!(
            ArtifactMetadata,
            "queries/artifact_metadata/upsert.sql",
            url,
            checksum,
            etag,
            last_modified,
        )
        .fetch_one(pool)
        .await
    }
}
