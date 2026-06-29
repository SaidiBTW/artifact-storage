-- Add up migration script here
CREATE TABLE artifact_metadata (
  -- Primary key using a secure, auto-generating UUID
  id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),

  -- The URL being tracked. TEXT is unlimited
  url TEXT NOT NULL UNIQUE,

  -- The checksum (SHA-256 hash)
  checksum TEXT,

  -- The ETag provided by the external servers HTTP headers
  etag TEXT,

  -- The last modified date procider by the external server
  -- TIMESTAMPZ
  last_modified TIMESTAMPTZ,

  --Internal tracking for creation and update
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_resources_url ON artifact_metadata(url);
