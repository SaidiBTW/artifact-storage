INSERT INTO  artifact_metadata(url, checksum, etag, last_modified)
VALUES ($1,$2,$3, NOW())
ON CONFLICT (url) DO UPDATE
SET etag = EXCLUDED.etag,
checksum = EXCLUDED.checksum,
updated_at = NOW()

RETURNING id, url, checksum, etag, last_modified, created_at, updated_at;