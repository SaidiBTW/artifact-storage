-- Add down migration script here
ALTER TABLE artifact_metadata
DROP CONSTRAINT IF EXISTS artifact_metadata_checksum_unique;
ALTER TABLE artifact_metadata
DROP CONSTRAINT IF EXISTS artifact_metadata_etag_unique;
