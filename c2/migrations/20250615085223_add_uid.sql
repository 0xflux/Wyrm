-- Add migration script here
ALTER TABLE agents
  ADD COLUMN uid TEXT;