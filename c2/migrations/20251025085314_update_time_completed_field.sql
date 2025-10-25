-- Add migration script here
ALTER TABLE completed_tasks
ALTER COLUMN time_completed DROP DEFAULT;