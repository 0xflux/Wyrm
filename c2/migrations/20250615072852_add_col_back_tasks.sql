-- Add migration script here
ALTER TABLE tasks
  ADD COLUMN command_id INT;