INSERT INTO  artifact_metadata(url, checksum, etag, last_modified)
VALUES ($1,$2,$3,$4)
ON CONFLICT (url) DO UPDATE
SET etag = EXCLUDED.etag,
checksum = EXCLUDED.checksum,
last_modified = $4,
updated_at = $4

RETURNING id, url, checksum, etag, last_modified, created_at, updated_at;