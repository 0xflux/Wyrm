-- Add migration script here
ALTER TABLE agent_staging
  ADD COLUMN xor_key smallint DEFAULT 0;