-- Add migration script here
ALTER TABLE completed_tasks
    ADD COLUMN agent_id TEXT;

ALTER TABLE completed_tasks
    ADD COLUMN command_id INT;