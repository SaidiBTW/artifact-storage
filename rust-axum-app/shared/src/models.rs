use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(sqlx::FromRow, Serialize, Deserialize, Debug)]
pub struct ArtifactMetadata {
    pub id: Uuid,
    pub url: String,
    pub checksum: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
