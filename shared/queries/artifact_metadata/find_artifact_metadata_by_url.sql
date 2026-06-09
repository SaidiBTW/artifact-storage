SELECT id, url,checksum, etag, last_modified, created_At, updated_at
        FROM artifact_metadata
        WHERE url = $1