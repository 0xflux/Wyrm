-- Add migration script here
ALTER TABLE agents
  ADD COLUMN pe_name TEXT;

UPDATE agents
  SET pe_name = 'oops'
  WHERE pe_name IS NULL;

ALTER TABLE agents
  ALTER COLUMN pe_name SET NOT NULL;