-- Add migration script here
ALTER TABLE completed_tasks
ADD COLUMN time_completed_ms BIGINT NOT NULL
    DEFAULT ((EXTRACT(EPOCH FROM now()) * 1000)::BIGINT);

UPDATE completed_tasks
SET time_completed_ms = ((EXTRACT(EPOCH FROM time_completed) * 1000)::BIGINT)
WHERE time_completed IS NOT NULL;