-- Add migration script here
ALTER TABLE agent_staging
    ADD COLUMN num_downloads INT NOT NULL DEFAULT 0;