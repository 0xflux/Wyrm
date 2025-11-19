-- Add migration script here
ALTER TABLE tasks
  ADD COLUMN fetched BOOL;