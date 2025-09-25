-- Add migration script here
ALTER TABLE agent_staging
  ADD COLUMN port INT NOT NULL;