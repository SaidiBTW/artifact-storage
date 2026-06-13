-- Add up migration script here
ALTER TABLE artifact_metadata 
ADD CONSTRAINT artifact_metadata_checksum_unique UNIQUE (checksum);

ALTER TABLE artifact_metadata 
ADD CONSTRAINT artifact_metadata_etag_unique UNIQUE (etag);