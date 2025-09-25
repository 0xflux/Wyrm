-- Add migration script here
ALTER TABLE agent_staging
  ADD COLUMN security_token TEXT NOT NULL;