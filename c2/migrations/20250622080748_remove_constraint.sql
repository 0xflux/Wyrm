-- Add migration script here
ALTER TABLE agent_staging
  DROP CONSTRAINT IF EXISTS uq_c2_endpoint;