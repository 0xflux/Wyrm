-- Add migration script here
ALTER TABLE agents
  ADD COLUMN last_check_in TIMESTAMPTZ DEFAULT now();