-- Credential versioning for safe key rotation.

ALTER TABLE external_credentials
    ADD COLUMN IF NOT EXISTS version INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS replaced_by VARCHAR(64);

CREATE INDEX IF NOT EXISTS idx_external_credentials_active
    ON external_credentials(owner, provider, label)
    WHERE revoked_at IS NULL;
